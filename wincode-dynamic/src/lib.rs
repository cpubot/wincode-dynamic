use wincode::{ReadResult, SchemaRead, SchemaWrite, error::invalid_tag_encoding, io::Reader};

mod ty;
mod value;
mod wincode_extra;
pub use {ty::*, value::*, wincode_dynamic_derive::*};

#[derive(SchemaRead, SchemaWrite, Debug, Clone)]
pub struct Field {
    name: String,
    ty: Ty,
    size: Option<usize>,
}

impl Field {
    pub fn new(name: impl Into<String>, ty: Ty, size: impl Into<Option<usize>>) -> Self {
        Self {
            name: name.into(),
            ty,
            size: size.into(),
        }
    }

    #[inline]
    pub fn parse<'de>(&self, reader: impl Reader<'de>) -> ReadResult<Value<'de>> {
        self.ty.parse(reader)
    }
}

pub enum RootSchema {
    Struct(Schema),
    Enum {
        variants: Box<[Schema]>,
        size: Option<usize>,
        name: String,
        tag_encoding: PrimitiveTy,
    },
}

#[derive(SchemaRead, SchemaWrite, Debug, Clone)]
pub struct Schema {
    name: String,
    fields: Box<[Field]>,
    size: Option<usize>,
}

impl Schema {
    pub fn new(
        name: impl Into<String>,
        fields: Box<[Field]>,
        size: impl Into<Option<usize>>,
    ) -> Self {
        Self {
            name: name.into(),
            fields,
            size: size.into(),
        }
    }
}

pub trait SchemaDynamic {
    fn schema() -> RootSchema;
}

pub struct SchemaRuntime {
    schema: RootSchema,
}

impl SchemaRuntime {
    pub fn new(schema: RootSchema) -> Self {
        Self { schema }
    }

    pub fn name(&self) -> &str {
        match &self.schema {
            RootSchema::Struct(schema) => &schema.name,
            RootSchema::Enum { name, .. } => name,
        }
    }

    pub fn size(&self) -> Option<usize> {
        match &self.schema {
            RootSchema::Struct(schema) => schema.size,
            RootSchema::Enum { size, .. } => *size,
        }
    }

    #[inline]
    pub fn fields<'a, 'de>(
        &'a self,
        mut reader: impl Reader<'de> + 'a,
    ) -> ReadResult<impl Iterator<Item = ReadResult<Value<'de>>> + 'a> {
        let fields = match &self.schema {
            RootSchema::Struct(schema) => &schema.fields,
            RootSchema::Enum {
                variants,
                tag_encoding,
                ..
            } => {
                let disc = tag_encoding.parse_into_usize(reader.by_ref())?;

                &variants
                    .get(disc)
                    .ok_or_else(|| invalid_tag_encoding(disc))?
                    .fields
            }
        };

        Ok(fields.iter().map(move |field| field.parse(reader.by_ref())))
    }
}

#[cfg(test)]
mod test {
    use {super::*, std::borrow::Cow};

    #[derive(SchemaDynamic, SchemaRead, SchemaWrite, PartialEq, Debug)]
    #[wincode_dynamic(internal)]
    struct StructMessage {
        a: u64,
        b: bool,
        vals: Vec<u64>,
        str: String,
        bytes: Vec<u8>,
        ar: [u64; 4],
        ar_bytes: [u8; 8],
    }

    #[derive(SchemaDynamic, SchemaRead, SchemaWrite)]
    #[wincode_dynamic(internal)]
    enum EnumMessage {
        Ping,
        Coordinates(u64, bool),
        Payload { text: String, bytes: Vec<u8> },
    }

    #[derive(SchemaDynamic, SchemaRead, SchemaWrite)]
    #[wincode_dynamic(internal)]
    #[wincode(tag_encoding = "u8")]
    enum U8EnumMessage {
        Ping,
        Value(u64),
    }

    fn assert_enum_message(
        runtime: &SchemaRuntime,
        message: &EnumMessage,
        expected: Vec<Value<'_>>,
    ) {
        let payload = wincode::serialize(message).unwrap();
        let actual = runtime
            .fields(payload.as_slice())
            .unwrap()
            .collect::<ReadResult<Vec<_>>>()
            .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn struct_roundtrip() {
        let message = StructMessage {
            a: 42,
            b: true,
            vals: vec![333; 4],
            str: String::from("hello world"),
            bytes: vec![42; 8],
            ar: [444; 4],
            ar_bytes: [43; 8],
        };

        let schema = StructMessage::schema();
        let dyn_parser = SchemaRuntime::new(schema);

        let payload = wincode::serialize(&message).unwrap();
        let result = dyn_parser
            .fields(&payload[..])
            .unwrap()
            .collect::<ReadResult<Vec<_>>>()
            .unwrap();
        assert_eq!(
            result,
            vec![
                Value::U64(42),
                Value::Bool(true),
                Value::Vec(vec![PrimitiveValue::U64(333); 4]),
                Value::String("hello world".into()),
                Value::Bytes(vec![42; 8].into()),
                Value::Vec(vec![PrimitiveValue::U64(444); 4]),
                Value::Bytes(vec![43; 8].into()),
            ]
        )
    }

    #[test]
    fn enum_schema() {
        let RootSchema::Enum {
            name,
            variants,
            size,
            tag_encoding,
        } = EnumMessage::schema()
        else {
            panic!("expected an enum schema");
        };

        assert_eq!(name, "EnumMessage");
        assert_eq!(size, None);
        assert_eq!(variants.len(), 3);
        assert_eq!(tag_encoding, PrimitiveTy::U32);

        assert_eq!(variants[0].name, "Ping");
        assert!(variants[0].fields.is_empty());
        assert_eq!(variants[0].size, Some(0));

        assert_eq!(variants[1].name, "Coordinates");
        assert_eq!(variants[1].fields[0].name, "0");
        assert_eq!(variants[1].fields[0].ty, Ty::PrimitiveTy(PrimitiveTy::U64));
        assert_eq!(variants[1].fields[1].name, "1");
        assert_eq!(variants[1].fields[1].ty, Ty::PrimitiveTy(PrimitiveTy::Bool));
        assert_eq!(variants[1].size, Some(9));

        assert_eq!(variants[2].name, "Payload");
        assert_eq!(variants[2].fields[0].name, "text");
        assert_eq!(variants[2].fields[0].ty, Ty::String);
        assert_eq!(variants[2].fields[1].name, "bytes");
        assert_eq!(
            variants[2].fields[1].ty,
            Ty::Vec {
                ty: PrimitiveTy::U8
            }
        );
        assert_eq!(variants[2].size, None);
    }

    #[test]
    fn enum_with_u8_tag_encoding_roundtrips() {
        let RootSchema::Enum { tag_encoding, .. } = U8EnumMessage::schema() else {
            panic!("expected an enum schema");
        };
        assert_eq!(tag_encoding, PrimitiveTy::U8);

        let runtime = SchemaRuntime::new(U8EnumMessage::schema());

        let ping = wincode::serialize(&U8EnumMessage::Ping).unwrap();
        assert_eq!(runtime.fields(ping.as_slice()).unwrap().count(), 0);

        let value = wincode::serialize(&U8EnumMessage::Value(42)).unwrap();
        let fields = runtime
            .fields(value.as_slice())
            .unwrap()
            .collect::<ReadResult<Vec<_>>>()
            .unwrap();
        assert_eq!(fields, vec![Value::U64(42)]);
    }

    #[test]
    fn enum_roundtrips_every_variant_shape() {
        let runtime = SchemaRuntime::new(EnumMessage::schema());

        assert_enum_message(&runtime, &EnumMessage::Ping, Vec::new());
        assert_enum_message(
            &runtime,
            &EnumMessage::Coordinates(42, true),
            vec![Value::U64(42), Value::Bool(true)],
        );
        assert_enum_message(
            &runtime,
            &EnumMessage::Payload {
                text: "hello".into(),
                bytes: vec![1, 2, 3, 4],
            },
            vec![
                Value::String(Cow::Owned("hello".into())),
                Value::Bytes(Cow::Owned(vec![1, 2, 3, 4])),
            ],
        );
    }

    #[test]
    fn enum_rejects_invalid_discriminant() {
        let runtime = SchemaRuntime::new(EnumMessage::schema());
        let payload = wincode::serialize(&u32::MAX).unwrap();

        let error = match runtime.fields(payload.as_slice()) {
            Ok(_) => panic!("invalid discriminant unexpectedly parsed"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            wincode::ReadError::InvalidTagEncoding(value) if value == u32::MAX as usize
        ));
    }

    #[test]
    fn enum_reports_truncated_and_malformed_fields() {
        let runtime = SchemaRuntime::new(EnumMessage::schema());

        let truncated_discriminant = [0u8; 3];
        assert!(runtime.fields(&truncated_discriminant[..]).is_err());

        let mut truncated = wincode::serialize(&EnumMessage::Coordinates(42, true)).unwrap();
        truncated.pop();
        let truncated_result = runtime
            .fields(truncated.as_slice())
            .unwrap()
            .collect::<ReadResult<Vec<_>>>();
        assert!(truncated_result.is_err());

        let mut malformed = wincode::serialize(&EnumMessage::Coordinates(42, true)).unwrap();
        *malformed.last_mut().unwrap() = 2;
        let malformed_result = runtime
            .fields(malformed.as_slice())
            .unwrap()
            .collect::<ReadResult<Vec<_>>>();
        assert!(matches!(
            malformed_result,
            Err(wincode::ReadError::InvalidBoolEncoding(2))
        ));
    }

    #[test]
    fn string_and_bytes_borrow_from_the_input() {
        #[derive(SchemaDynamic, SchemaRead, SchemaWrite)]
        #[wincode_dynamic(internal)]
        struct Borrowable {
            text: String,
            bytes: Vec<u8>,
        }

        let value = Borrowable {
            text: "borrow me".into(),
            bytes: vec![5, 6, 7, 8],
        };
        let payload = wincode::serialize(&value).unwrap();
        let runtime = SchemaRuntime::new(Borrowable::schema());
        let fields = runtime
            .fields(payload.as_slice())
            .unwrap()
            .collect::<ReadResult<Vec<_>>>()
            .unwrap();

        match fields.as_slice() {
            [
                Value::String(Cow::Borrowed(text)),
                Value::Bytes(Cow::Borrowed(bytes)),
            ] => {
                assert_eq!(*text, "borrow me");
                assert_eq!(*bytes, [5, 6, 7, 8]);
            }
            fields => panic!("expected borrowed string and bytes, got {fields:?}"),
        }
    }

    #[test]
    fn generic_enum_schema_and_roundtrip() {
        #[derive(SchemaDynamic, SchemaRead, SchemaWrite)]
        #[wincode_dynamic(internal)]
        enum Generic<T> {
            Empty,
            Item(T),
        }

        let runtime = SchemaRuntime::new(Generic::<u64>::schema());
        let payload = wincode::serialize(&Generic::Item(77u64)).unwrap();
        let fields = runtime
            .fields(payload.as_slice())
            .unwrap()
            .collect::<ReadResult<Vec<_>>>()
            .unwrap();

        assert_eq!(fields, vec![Value::U64(77)]);

        let empty_payload = wincode::serialize(&Generic::<u64>::Empty).unwrap();
        assert_eq!(runtime.fields(empty_payload.as_slice()).unwrap().count(), 0);
    }
}
