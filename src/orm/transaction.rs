use crate::driver::DatabaseDriver;
use std::error::Error;

pub struct Transaction<'a> {
    driver: &'a mut dyn DatabaseDriver,
    active: bool,
}

impl<'a> Transaction<'a> {
    pub fn begin(driver: &'a mut dyn DatabaseDriver) -> Result<Self, Box<dyn Error>> {
        driver.begin()?;
        Ok(Transaction { driver, active: true })
    }

    pub fn commit(mut self) -> Result<(), Box<dyn Error>> {
        if self.active {
            self.driver.commit()?;
            self.active = false;
        }
        Ok(())
    }

    pub fn rollback(mut self) -> Result<(), Box<dyn Error>> {
        if self.active {
            self.driver.rollback()?;
            self.active = false;
        }
        Ok(())
    }

    pub fn driver(&mut self) -> &mut dyn DatabaseDriver {
        self.driver
    }
}

impl<'a> Drop for Transaction<'a> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.driver.rollback();
        }
    }
}