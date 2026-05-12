use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Data, Fields, Meta};

#[proc_macro_derive(Model, attributes(table, column, primary_key))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let table_name = get_attr(&input.attrs, "table")
        .unwrap_or_else(|| name.to_string().to_lowercase() + "s");

    let primary_key = get_attr(&input.attrs, "primary_key")
        .unwrap_or_else(|| "id".to_string());

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("Model derive only supports named fields"),
        },
        _ => panic!("Model derive only supports structs"),
    };

    let field_names: Vec<_> = fields.iter()
        .map(|f| f.ident.as_ref().unwrap().to_string())
        .collect();

    let field_names_ref: Vec<_> = field_names.iter().map(|n| n.as_str()).collect();

    let from_row_arms = fields.iter().enumerate().map(|(i, f)| {
        let fname = f.ident.as_ref().unwrap();
        let ty = &f.ty;
        quote! {
            #fname: {
                let val = &row[#i];
                <#ty as ::grorm::FromSql>::from_sql(val)
                    .map_err(|e| format!("field '{}': {}", stringify!(#fname), e))?
            },
        }
    });

    let to_values_entries = fields.iter().map(|f| {
        let fname = f.ident.as_ref().unwrap();
        quote! {
            ::grorm::ToSql::to_sql(&self.#fname)
        }
    });

    let schema_entries = fields.iter().map(|f| {
        let fname = f.ident.as_ref().unwrap();
        let fname_str = fname.to_string();
        let ty = &f.ty;
        let ty_str = quote! { #ty }.to_string();
        let is_pk = fname_str == primary_key;
        let is_auto = is_pk && is_integer_type(&ty_str);
        quote! {
            ::grorm::ColumnInfo {
                name: #fname_str,
                rust_type: #ty_str,
                is_primary_key: #is_pk,
                is_auto_increment: #is_auto,
            }
        }
    });

    let expanded = quote! {
        impl ::grorm::Model for #name {
            fn table_name() -> &'static str {
                #table_name
            }

            fn primary_key() -> &'static str {
                #primary_key
            }

            fn columns() -> &'static [&'static str] {
                &[#(#field_names_ref),*]
            }

            fn table_schema() -> &'static [::grorm::ColumnInfo] {
                &[#(#schema_entries),*]
            }

            fn from_row(row: &[::grorm::Value]) -> Result<Self, String> {
                Ok(Self {
                    #(#from_row_arms)*
                })
            }

            fn to_values(&self) -> Vec<::grorm::Value> {
                vec![#(#to_values_entries),*]
            }
        }
    };

    TokenStream::from(expanded)
}

fn is_integer_type(ty: &str) -> bool {
    ty == "i8" || ty == "i16" || ty == "i32" || ty == "i64"
        || ty == "u8" || ty == "u16" || ty == "u32" || ty == "u64"
        || ty == "isize" || ty == "usize"
}

#[proc_macro_derive(Table, attributes(table_name))]
pub fn derive_table(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let table_name = get_attr(&input.attrs, "table_name")
        .unwrap_or_else(|| name.to_string().to_lowercase() + "s");

    let expanded = quote! {
        impl #name {
            pub fn table_name() -> &'static str {
                #table_name
            }
        }
    };

    TokenStream::from(expanded)
}

fn get_attr(attrs: &[syn::Attribute], name: &str) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident(name) {
            if let Meta::NameValue(mnv) = &attr.meta {
                if let syn::Expr::Lit(lit) = &mnv.value {
                    if let syn::Lit::Str(s) = &lit.lit {
                        return Some(s.value());
                    }
                }
            }
        }
    }
    None
}