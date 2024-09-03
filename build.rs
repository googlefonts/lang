use proc_macro2::TokenStream;
use protobuf::reflect::{FieldDescriptor, ReflectValueRef};
use quote::{quote, format_ident};
use std::io::{BufWriter, Write};
use std::{env, fs::File, path::Path};
use itertools::Itertools;

fn main() {
    let mut config = prost_build::Config::new();
    config.compile_well_known_types();
    config.boxed(".google.languages_public.LanguageProto.sample_text");
    config.boxed(".google.languages_public.LanguageProto.exemplar_chars");
    config
        .compile_protos(
            &["Lib/gflanguages/languages_public.proto"],
            &["Lib/gflanguages/"],
        )
        .expect("Could not compile languages_public.proto");

    let descriptors = protobuf_parse::Parser::new()
        .pure()
        .include(".")
        .input("Lib/gflanguages/languages_public.proto")
        .file_descriptor_set()
        .expect("Could not parse languages_public.proto");
    let protofile = descriptors.file.first().expect("No file in descriptor");
    let descriptor = protobuf::reflect::FileDescriptor::new_dynamic(protofile.clone(), &[])
        .expect("Could not create descriptor");

    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("data.rs");
    let mut file = BufWriter::new(File::create(path).unwrap());
    let mut output = quote! { use std::collections::BTreeMap; use std::sync::LazyLock; };

    output.extend(serialize_a_structure(
        ".google.languages_public.RegionProto",
        "Lib/gflanguages/data/regions/*.textproto",
        "REGIONS",
        &descriptor,
    ));

    output.extend(serialize_a_structure(
        ".google.languages_public.ScriptProto",
        "Lib/gflanguages/data/scripts/*.textproto",
        "SCRIPTS",
        &descriptor,
    ));

    output.extend(serialize_a_structure(
        ".google.languages_public.LanguageProto",
        "Lib/gflanguages/data/languages/*.textproto",
        "LANGUAGES",
        &descriptor,
    ));

    let abstract_file: syn::File = syn::parse2(output).expect("Could not parse output");
    let formatted = prettyplease::unparse(&abstract_file);
    file.write_all(formatted.as_bytes())
        .expect("Could not write to file");
}


fn serialize_a_structure(proto_name: &str, pathglob: &str, output_variable: &str, descriptor: &protobuf::reflect::FileDescriptor) -> TokenStream {
    let proto = descriptor
        .message_by_full_name(proto_name)
        .unwrap_or_else(|| panic!("No {} message", proto_name));
    let files: Vec<std::path::PathBuf> =
        glob::glob(pathglob)
            .expect("Failed to read glob pattern")
            .flatten()
            .collect();
    let name: TokenStream = proto.name().parse().unwrap();
    let variable: TokenStream = output_variable.parse().unwrap();
    // We can't fill the BTreeMap in one go, because a massive function
    // definition (>100k) will cause a stack overflow. So we split it into
    // chunks of 400, a reasonable size, write each chunk out as a separate
    // function, and call them all in the main lazylock function.
    let (definitions, calls): (TokenStream, TokenStream) = files
        .into_iter()
        .map(|file| serialize_file(file, &proto))
        .chunks(400)
        .into_iter()
        .enumerate()
        .map(|(index, tokens)| {
            let fn_name = format_ident!("fill_{}_{}", proto.name(), index);
            (quote! {
                #[allow(non_snake_case)]
                fn #fn_name(data: &mut BTreeMap<&str, Box<#name>>) {
                    #(#tokens)*
                }
            },
            quote!{
                #fn_name(&mut data);
            })
        })
        .collect();
    let docmsg = format!("A map of all the {} objects", name);
    quote! {
        #definitions
        #[doc = #docmsg]
        pub static #variable: LazyLock<BTreeMap<&str, Box<#name>>> = LazyLock::new(|| {
            let mut data = BTreeMap::new();
            #calls
            data
        });
    }
}
fn serialize_file(
    path: std::path::PathBuf,
    descriptor: &protobuf::reflect::MessageDescriptor,
) -> TokenStream {
    let mut message = descriptor.new_instance();
    let message_mut = message.as_mut();
    let input = std::fs::read_to_string(&path).expect("Could not read file");
    protobuf::text_format::merge_from_str(message_mut, &input)
        .unwrap_or_else(|e| panic!("Could not parse file {:?}: {:?}", path, e));
    let id = path.file_stem().unwrap().to_str().unwrap();
    let serialized = serialize_message(message_mut);
    quote!(
        data.insert(#id, Box::new(#serialized));
    )
}

fn serialize_message(message: &dyn protobuf::MessageDyn) -> TokenStream {
    let descriptor = message.descriptor_dyn();
    let descriptor_name: TokenStream = descriptor.name().parse().unwrap();
    let fields = descriptor.fields().map(|field| {
        let field_name: TokenStream = field.name().parse().unwrap();
        let field_contents = serialize_field(&field, message);
        quote!(
           #field_name: #field_contents
        )
    });
    quote!(
        #descriptor_name {
            #(#fields),*
        }
    )
}

fn serialize_field(field: &FieldDescriptor, message: &dyn protobuf::MessageDyn) -> TokenStream {
    if field.is_repeated() {
        let values = field.get_repeated(message).into_iter().map(|value| {
            let value = serialize_field_value(field, value);
            quote!(#value)
        });
        quote!(vec![#(#values),*])
    } else if field.is_required() {
        serialize_field_value(field, field.get_singular(message).unwrap())
    } else if field.has_field(message) {
        let value = serialize_field_value(field, field.get_singular(message).unwrap());
        quote!(Some(#value))
    } else {
        quote!(None)
    }
}

fn serialize_field_value(_field: &FieldDescriptor, value: ReflectValueRef) -> TokenStream {
    match value {
        ReflectValueRef::I32(value) => quote!(#value),
        ReflectValueRef::I64(value) => quote!(#value),
        ReflectValueRef::U32(value) => quote!(#value),
        ReflectValueRef::U64(value) => quote!(#value),
        ReflectValueRef::F32(value) => quote!(#value),
        ReflectValueRef::F64(value) => quote!(#value),
        ReflectValueRef::Bool(value) => quote!(#value),
        ReflectValueRef::String(value) => {
            quote!(#value.to_string())
        }
        ReflectValueRef::Enum(_value, _key) => {
            unimplemented!()
        }
        ReflectValueRef::Message(value) => {
            let message = serialize_message(&*value);
            quote!(Box::new(#message))
        }
        ReflectValueRef::Bytes(_value) => {
            unimplemented!()
        }
    }
}
