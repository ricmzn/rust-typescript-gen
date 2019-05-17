extern crate proc_macro;

use std::io::Write;
use std::fs::File;
use proc_macro::TokenStream;
use syn::PathArguments::AngleBracketed;
use syn::Data::Struct;
use syn::*;

trait UnwrapOrErr<T> {
    fn unwrap_or_err(self) -> T;
}

impl<T1, T2, T> UnwrapOrErr<T> for std::result::Result<T1, T2>
where
    T1: Into<T>,
    T2: Into<T>
{
    fn unwrap_or_err(self) -> T {
        match self {
            Ok(result) => result.into(),
            Err(error) => error.into()
        }
    }
}

struct GenericType {
    name: String,
    args: Option<Box<GenericType>>
}

fn rust_type_to_typescript(field: &Field, rs_type: GenericType) -> Result<String> {
    match rs_type.name.as_ref() {
        "String" | "str" => Ok("string".into()),
        "i16" | "i32" | "u16" | "u32" => Ok("number".into()),
        "f32" | "f64" => Ok("number".into()),
        "bool" => Ok("boolean".into()),
        "Slice" => Ok(format!("{}[]", rust_type_to_typescript(field, *rs_type.args.unwrap())?)),
        "Vec" => Ok(format!("{}[]", rust_type_to_typescript(field, *rs_type.args.unwrap())?)),
        "Box" => Ok(rust_type_to_typescript(field, *rs_type.args.unwrap())?),
        "None" => Ok("null".into()),
        "Option" => Ok(format!("{} | null", rust_type_to_typescript(field, *rs_type.args.unwrap())?)),
        ty => Err(Error::new_spanned(field, format!("Unmapped type: {}", ty)))
    }
}

fn extract_generic_type(generic_args: &AngleBracketedGenericArguments) -> Result<GenericType> {
    let segment = &generic_args.args.iter().nth(0).unwrap();
    match segment {
        GenericArgument::Type(ty) => get_type(ty),
        _ => panic!("Unknown generic type segment for {:#?}", generic_args)
    }
}

fn get_type(ty: &Type) -> Result<GenericType> {
    match ty {
        Type::Reference(ty) => get_type(&ty.elem),
        Type::Path(ty) => {
            let first_segment = &ty.path.segments.first();
            match first_segment.as_ref() {
                Some(segment) => {
                    let value = segment.value();
                    match &value.arguments {
                        AngleBracketed(args) => Ok(GenericType {
                            name: value.ident.to_string(),
                            args: Some(Box::new(extract_generic_type(args).unwrap()))
                        }),
                        _ => Ok(GenericType {
                            name: value.ident.to_string(),
                            args: None
                        })
                    }
                },
                None => Err(Error::new_spanned(ty, "No type found"))
            }
        },
        Type::Slice(ty) => Ok(GenericType {
            name: "Slice".into(),
            args: Some(Box::new(get_type(ty.elem.as_ref())?))
        }),
        Type::Tuple(ty) => {
            match ty.elems.iter().nth(0) {
                Some(ty) => get_type(ty),
                None => Ok(GenericType {
                    name: "None".into(),
                    args: None
                })
            }
        },
        _ => Err(Error::new_spanned(ty, "Error getting type name"))
    }
}

fn get_type_and_name(field: &Field) -> Result<(GenericType, String)> {
    let name = field.ident.as_ref().unwrap().to_string();
    let ty = get_type(&field.ty)?;
    Ok((ty, name))
}

fn emit_typescript_interface(name: String, data: &DataStruct) -> Result<()> {
    let mut file = File::create("target/types.d.ts").unwrap();
    writeln!(file, "interface {} {{", name).unwrap();
    for field in data.fields.iter() {
        let (ty, name) = get_type_and_name(field)?;
        writeln!(file, "    {}: {};", name, rust_type_to_typescript(field, ty)?).unwrap();
    }
    writeln!(file, "}}").unwrap();
    Ok(())
}

#[proc_macro_derive(TypescriptInterface)]
pub fn derive_typescript_interface(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse(input).unwrap();
    match input.data {
        Struct(data) => emit_typescript_interface(input.ident.to_string(), &data)
            .map_err(|e| e.to_compile_error())
            .map(|_| TokenStream::new())
            .unwrap_or_err(),
        _ => Error::new_spanned(input, "Expected struct for #[derive(TypescriptInterface)]")
            .to_compile_error()
            .into()
    }
}
