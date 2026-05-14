use gorust::net::AsyncTcpStream;
use md5::{Digest, Md5};
use std::net::SocketAddr;

const PG_PROTOCOL_VERSION: i32 = 196608;

#[derive(Debug, Clone)]
pub struct PgColumnInfo {
    pub name: String,
    pub table_oid: i32,
    pub column_attr: i16,
    pub data_type: i32,
    pub type_size: i16,
    pub type_modifier: i32,
    pub format_code: i16,
}

pub enum PgResult {
    Rows(Vec<Vec<String>>, Vec<PgColumnInfo>),
    CommandComplete(String),
    Empty,
}

pub struct PgConnection {
    stream: AsyncTcpStream,
    username: String,
    password: String,
}

impl PgConnection {
    pub fn connect(
        addr: SocketAddr,
        username: &str,
        password: &str,
        database: &str,
    ) -> Result<Self, crate::error::Error> {
        let stream = AsyncTcpStream::connect(addr)?;
        let mut conn = PgConnection {
            stream,
            username: username.to_string(),
            password: password.to_string(),
        };
        conn.send_startup_message(username, database)?;
        conn.read_authentication()?;
        conn.read_until_ready()?;
        Ok(conn)
    }

    fn send_startup_message(
        &mut self,
        username: &str,
        database: &str,
    ) -> Result<(), crate::error::Error> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&PG_PROTOCOL_VERSION.to_be_bytes());
        buf.extend_from_slice(b"user\0");
        buf.extend_from_slice(username.as_bytes());
        buf.push(0);
        buf.extend_from_slice(b"database\0");
        buf.extend_from_slice(database.as_bytes());
        buf.push(0);
        buf.push(0);

        let len = (buf.len() + 4) as i32;
        let mut msg = Vec::new();
        msg.extend_from_slice(&len.to_be_bytes());
        msg.extend_from_slice(&buf);
        self.stream.write_all(&msg)?;
        Ok(())
    }

    fn read_authentication(&mut self) -> Result<(), crate::error::Error> {
        loop {
            let msg_type = self.read_byte()?;
            let len = self.read_i32()?;

            match msg_type {
                b'R' => {
                    let auth_type = self.read_i32()?;
                    match auth_type {
                        0 => break,
                        5 => {
                            let mut salt = [0u8; 4];
                            self.stream.read(&mut salt)?;

                            let mut hasher = Md5::new();
                            hasher.update(self.password.as_bytes());
                            hasher.update(self.username.as_bytes());
                            let inner_hash = hex::encode(hasher.finalize_reset());

                            hasher.update(inner_hash.as_bytes());
                            hasher.update(&salt);
                            let outer_hash = hex::encode(hasher.finalize());

                            let md5_password = format!("md5{}", outer_hash);
                            self.send_password_message(&md5_password)?;
                        }
                        _ => {}
                    }
                }
                b'K' => {
                    let _pid = self.read_i32()?;
                    let _secret = self.read_i32()?;
                }
                b'S' | b'N' => {
                    self.skip_bytes((len - 4) as usize)?;
                }
                b'E' => {
                    let mut err_buf = vec![0u8; (len - 4) as usize];
                    self.stream.read(&mut err_buf)?;
                    let err_str = String::from_utf8_lossy(&err_buf);
                    return Err(format!("PostgreSQL error: {}", err_str).into());
                }
                _ => {
                    self.skip_bytes((len - 4) as usize)?;
                }
            }
        }
        Ok(())
    }

    fn send_password_message(&mut self, password: &str) -> Result<(), crate::error::Error> {
        let mut buf = Vec::new();
        buf.push(b'p');
        let content = format!("{}\0", password);
        let len = (content.len() + 4) as i32;
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(content.as_bytes());
        self.stream.write_all(&buf)?;
        Ok(())
    }

    fn read_until_ready(&mut self) -> Result<(), crate::error::Error> {
        loop {
            let msg_type = self.read_byte()?;
            let len = self.read_i32()?;

            match msg_type {
                b'Z' => {
                    let _status = self.read_byte()?;
                    break;
                }
                b'K' | b'S' | b'N' | b'T' | b'D' | b'C' => {
                    self.skip_bytes((len - 4) as usize)?;
                }
                b'E' => {
                    let mut err_buf = vec![0u8; (len - 4) as usize];
                    self.stream.read(&mut err_buf)?;
                    return Err(
                        format!("PostgreSQL error: {}", String::from_utf8_lossy(&err_buf)).into(),
                    );
                }
                _ => {
                    self.skip_bytes((len - 4) as usize)?;
                }
            }
        }
        Ok(())
    }

    pub fn execute_query(&mut self, sql: &str) -> Result<PgResult, crate::error::Error> {
        self.send_query(sql)?;
        self.read_query_result()
    }

    fn send_query(&mut self, sql: &str) -> Result<(), crate::error::Error> {
        let mut buf = Vec::new();
        buf.push(b'Q');
        let content = format!("{}\0", sql);
        let len = (content.len() + 4) as i32;
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(content.as_bytes());
        self.stream.write_all(&buf)?;
        Ok(())
    }

    fn read_query_result(&mut self) -> Result<PgResult, crate::error::Error> {
        let mut columns: Vec<PgColumnInfo> = Vec::new();
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut command_tag = String::new();

        loop {
            let msg_type = self.read_byte()?;
            let len = self.read_i32()?;

            match msg_type {
                b'T' => {
                    let num_cols = self.read_i16()?;
                    columns = Vec::with_capacity(num_cols as usize);
                    for _ in 0..num_cols {
                        let mut name_buf = Vec::new();
                        loop {
                            let b = self.read_byte()?;
                            if b == 0 {
                                break;
                            }
                            name_buf.push(b);
                        }
                        let name = String::from_utf8_lossy(&name_buf).to_string();
                        let table_oid = self.read_i32()?;
                        let column_attr = self.read_i16()?;
                        let data_type = self.read_i32()?;
                        let type_size = self.read_i16()?;
                        let type_modifier = self.read_i32()?;
                        let format_code = self.read_i16()?;

                        columns.push(PgColumnInfo {
                            name,
                            table_oid,
                            column_attr,
                            data_type,
                            type_size,
                            type_modifier,
                            format_code,
                        });
                    }
                }
                b'D' => {
                    let num_cols = self.read_i16()?;
                    let mut row = Vec::with_capacity(num_cols as usize);
                    for _ in 0..num_cols {
                        let val_len = self.read_i32()?;
                        if val_len == -1 {
                            row.push("NULL".to_string());
                        } else {
                            let mut val_buf = vec![0u8; val_len as usize];
                            self.stream.read(&mut val_buf)?;
                            row.push(String::from_utf8_lossy(&val_buf).to_string());
                        }
                    }
                    rows.push(row);
                }
                b'C' => {
                    let mut tag_buf = Vec::new();
                    for _ in 0..(len - 4) {
                        let b = self.read_byte()?;
                        if b == 0 {
                            break;
                        }
                        tag_buf.push(b);
                    }
                    command_tag = String::from_utf8_lossy(&tag_buf).to_string();
                }
                b'Z' => {
                    let _status = self.read_byte()?;
                    break;
                }
                b'E' => {
                    let mut err_buf = vec![0u8; (len - 4) as usize];
                    self.stream.read(&mut err_buf)?;
                    return Err(
                        format!("PostgreSQL error: {}", String::from_utf8_lossy(&err_buf)).into(),
                    );
                }
                b'N' => {
                    self.skip_bytes((len - 4) as usize)?;
                }
                _ => {
                    self.skip_bytes((len - 4) as usize)?;
                }
            }
        }

        if !columns.is_empty() {
            Ok(PgResult::Rows(rows, columns))
        } else if !command_tag.is_empty() {
            Ok(PgResult::CommandComplete(command_tag))
        } else {
            Ok(PgResult::Empty)
        }
    }

    pub fn prepare(&mut self, name: &str, sql: &str) -> Result<(), crate::error::Error> {
        let mut buf = Vec::new();
        buf.push(b'P');
        let stmt_name = format!("{}\0", name);
        let query = format!("{}\0", sql);
        let num_params: i16 = 0;
        let content_len = stmt_name.len() + query.len() + 2;
        let len = (content_len + 4) as i32;
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(stmt_name.as_bytes());
        buf.extend_from_slice(query.as_bytes());
        buf.extend_from_slice(&num_params.to_be_bytes());
        self.stream.write_all(&buf)?;

        let mut buf = Vec::new();
        buf.push(b'D');
        buf.push(b'S');
        buf.push(0);
        let len: i32 = 6;
        buf.extend_from_slice(&len.to_be_bytes());
        buf.push(0);
        self.stream.write_all(&buf)?;

        self.read_until_ready()?;
        Ok(())
    }

    pub fn execute_prepared(
        &mut self,
        name: &str,
        params: &[&str],
    ) -> Result<PgResult, crate::error::Error> {
        let mut buf = Vec::new();
        buf.push(b'B');
        let portal = "\0";
        let stmt = format!("{}\0", name);
        let num_formats: i16 = 0;
        let num_params = params.len() as i16;

        let mut content = Vec::new();
        content.extend_from_slice(portal.as_bytes());
        content.extend_from_slice(stmt.as_bytes());
        content.extend_from_slice(&num_formats.to_be_bytes());
        content.extend_from_slice(&num_params.to_be_bytes());

        for p in params {
            if p.is_empty() || *p == "NULL" {
                content.extend_from_slice(&(-1i32).to_be_bytes());
            } else {
                let pbytes = p.as_bytes();
                content.extend_from_slice(&(pbytes.len() as i32).to_be_bytes());
                content.extend_from_slice(pbytes);
            }
        }
        content.extend_from_slice(&num_formats.to_be_bytes());

        let len = (content.len() + 4) as i32;
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(&content);
        self.stream.write_all(&buf)?;

        let mut buf = Vec::new();
        buf.push(b'E');
        let portal_name = "\0";
        let max_rows: i32 = 0;
        let content_len = portal_name.len() + 4;
        let len = (content_len + 4) as i32;
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(portal_name.as_bytes());
        buf.extend_from_slice(&max_rows.to_be_bytes());
        self.stream.write_all(&buf)?;

        let mut buf = Vec::new();
        buf.push(b'S');
        let len: i32 = 4;
        buf.extend_from_slice(&len.to_be_bytes());
        self.stream.write_all(&buf)?;

        self.read_query_result()
    }

    fn read_byte(&mut self) -> Result<u8, crate::error::Error> {
        let mut buf = [0u8; 1];
        self.stream.read(&mut buf)?;
        Ok(buf[0])
    }

    fn read_i16(&mut self) -> Result<i16, crate::error::Error> {
        let mut buf = [0u8; 2];
        self.stream.read(&mut buf)?;
        Ok(i16::from_be_bytes(buf))
    }

    fn read_i32(&mut self) -> Result<i32, crate::error::Error> {
        let mut buf = [0u8; 4];
        self.stream.read(&mut buf)?;
        Ok(i32::from_be_bytes(buf))
    }

    fn skip_bytes(&mut self, count: usize) -> Result<(), crate::error::Error> {
        let mut buf = vec![0u8; count];
        self.stream.read(&mut buf)?;
        Ok(())
    }
}
