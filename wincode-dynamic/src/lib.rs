#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use {
    alloc::{borrow::Cow, boxed::Box, string::String},
    wincode::{ReadResult, SchemaRead, SchemaWrite, error::invalid_tag_encoding, io::Reader},
};

mod ty;
mod value;
mod wincode_extra;
pub use {ty::*, value::*, wincode_dynamic_derive::*};

#[derive(SchemaRead, SchemaWrite, Debug, Clone)]
pub struct FieldDef {
    name: String,
    ty: Ty,
    size: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct Field<'meta, 'data> {
    name: Cow<'meta, str>,
    ty: Ty,
    size: Option<usize>,
    value: Value<'data>,
}

impl<'meta, 'data> Field<'meta, 'data> {
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    pub fn ty(&self) -> Ty {
        self.ty
    }

    #[inline]
    pub fn size(&self) -> Option<usize> {
        self.size
    }

    #[inline]
    pub fn value(&self) -> &Value<'data> {
        &self.value
    }

    #[inline]
    pub fn into_value(self) -> Value<'data> {
        self.value
    }
}

impl FieldDef {
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

#[derive(SchemaRead, SchemaWrite, Debug, Clone)]
#[wincode(tag_encoding = "u8")]
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
    fields: Box<[FieldDef]>,
    size: Option<usize>,
}

impl Schema {
    pub fn new(
        name: impl Into<String>,
        fields: Box<[FieldDef]>,
        size: impl Into<Option<usize>>,
    ) -> Self {
        Self {
            name: name.into(),
            fields,
            size: size.into(),
        }
    }

    #[inline]
    pub const fn size(&self) -> Option<usize> {
        self.size
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    pub const fn field_defs(&self) -> &[FieldDef] {
        &self.fields
    }
}

pub trait SchemaDynamic {
    fn schema() -> RootSchema;
}

#[derive(Debug)]
pub struct Decoder {
    schema: RootSchema,
}

impl Decoder {
    pub fn new(schema: RootSchema) -> Self {
        Self { schema }
    }

    #[inline]
    pub fn name(&self) -> &str {
        match &self.schema {
            RootSchema::Struct(schema) => &schema.name,
            RootSchema::Enum { name, .. } => name,
        }
    }

    #[inline]
    pub const fn size(&self) -> Option<usize> {
        match &self.schema {
            RootSchema::Struct(schema) => schema.size,
            RootSchema::Enum { size, .. } => *size,
        }
    }

    #[inline]
    pub fn fields<'a, 'de>(
        &'a self,
        mut reader: impl Reader<'de> + 'a,
    ) -> ReadResult<impl Iterator<Item = ReadResult<Field<'a, 'de>>> + 'a> {
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

        Ok(fields.iter().map(move |field| {
            let value = field.parse(reader.by_ref())?;
            Ok(Field {
                name: Cow::Borrowed(&field.name),
                ty: field.ty,
                size: field.size,
                value,
            })
        }))
    }
}

#[cfg(all(test, feature = "std"))]
mod test {
    use {super::*, proptest::prelude::*, proptest_derive::Arbitrary};

    #[derive(Arbitrary, SchemaDynamic, SchemaRead, SchemaWrite, PartialEq, Debug)]
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

    #[derive(Arbitrary, SchemaDynamic, SchemaRead, SchemaWrite, Debug)]
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
        decoder: &Decoder,
        message: &EnumMessage,
        expected: Vec<(&str, Ty, Option<usize>, Value<'_>)>,
    ) {
        let payload = wincode::serialize(message).unwrap();
        let actual = decoder
            .fields(payload.as_slice())
            .unwrap()
            .collect::<ReadResult<Vec<_>>>()
            .unwrap();
        assert_eq!(actual.len(), expected.len());
        for (actual, (name, ty, size, value)) in actual.iter().zip(expected.iter()) {
            assert_field(actual, name, *ty, *size, value);
        }
    }

    fn assert_field(
        field: &Field<'_, '_>,
        name: &str,
        ty: Ty,
        size: Option<usize>,
        value: &Value<'_>,
    ) {
        assert_eq!(field.name(), name);
        assert_eq!(field.ty(), ty);
        assert_eq!(field.size(), size);
        assert_eq!(field.value(), value);
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
        let decoder = Decoder::new(schema);

        let payload = wincode::serialize(&message).unwrap();
        let result = decoder
            .fields(&payload[..])
            .unwrap()
            .collect::<ReadResult<Vec<_>>>()
            .unwrap();
        let mut result = result.into_iter();

        let field = result.next().unwrap();
        assert_field(
            &field,
            "a",
            Ty::PrimitiveTy(PrimitiveTy::U64),
            Some(8),
            &Value::U64(42),
        );
        let field = result.next().unwrap();
        assert_field(
            &field,
            "b",
            Ty::PrimitiveTy(PrimitiveTy::Bool),
            Some(1),
            &Value::Bool(true),
        );

        let field = result.next().unwrap();
        assert_eq!(field.name(), "vals");
        assert_eq!(
            field.ty(),
            Ty::Vec {
                ty: PrimitiveTy::U64
            }
        );
        assert_eq!(field.size(), None);
        let Value::Vec(vals) = field.value else {
            panic!("expected vals to be a lazy vector");
        };
        assert_eq!(vals.ty(), PrimitiveTy::U64);
        assert_eq!(vals.len(), 4);
        assert!(!vals.is_empty());
        assert_eq!(
            vals.clone().into_dyn_vec().unwrap(),
            vec![PrimitiveValue::U64(333); 4]
        );
        assert_eq!(
            vals.clone()
                .try_into_iter_as::<u64>()
                .unwrap()
                .collect::<ReadResult<Vec<_>>>()
                .unwrap(),
            vec![333; 4]
        );
        assert!(vals.try_into_iter_as::<u32>().is_err());

        let field = result.next().unwrap();
        assert_field(
            &field,
            "str",
            Ty::String,
            None,
            &Value::String("hello world".into()),
        );
        let field = result.next().unwrap();
        assert_field(
            &field,
            "bytes",
            Ty::Vec {
                ty: PrimitiveTy::U8,
            },
            None,
            &Value::Bytes(vec![42; 8].into()),
        );

        let field = result.next().unwrap();
        assert_eq!(field.name(), "ar");
        assert_eq!(
            field.ty(),
            Ty::Array {
                ty: PrimitiveTy::U64,
                len: 4
            }
        );
        assert_eq!(field.size(), Some(32));
        let Value::Vec(vals) = field.value else {
            panic!("expected ar to be a lazy vector");
        };
        assert_eq!(
            vals.try_into_iter_as::<u64>()
                .unwrap()
                .collect::<ReadResult<Vec<_>>>()
                .unwrap(),
            vec![444; 4]
        );

        let field = result.next().unwrap();
        assert_field(
            &field,
            "ar_bytes",
            Ty::Array {
                ty: PrimitiveTy::U8,
                len: 8,
            },
            Some(8),
            &Value::Bytes(vec![43; 8].into()),
        );
        assert!(result.next().is_none());
    }

    #[test]
    fn lazy_vector_reports_element_errors_during_iteration() {
        #[derive(SchemaDynamic, SchemaRead, SchemaWrite)]
        #[wincode_dynamic(internal)]
        struct Bools {
            values: Vec<bool>,
        }

        let mut payload = wincode::serialize(&Bools {
            values: vec![true, false],
        })
        .unwrap();
        *payload.last_mut().unwrap() = 2;

        let decoder = Decoder::new(Bools::schema());
        let value = decoder
            .fields(payload.as_slice())
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        let Value::Vec(values) = value.value else {
            panic!("expected a lazy vector");
        };

        let error = values
            .try_into_iter_as::<bool>()
            .unwrap()
            .collect::<ReadResult<Vec<_>>>()
            .unwrap_err();
        assert!(matches!(error, wincode::ReadError::InvalidBoolEncoding(2)));
    }

    #[test]
    fn owned_lazy_payloads_enforce_the_byte_preallocation_limit() {
        use wincode::{
            config::{Config, DEFAULT_PREALLOCATION_SIZE_LIMIT, DefaultConfig},
            io::Cursor,
            len::SeqLen,
        };

        type LengthEncoding = <DefaultConfig as Config>::LengthEncoding;

        let len = DEFAULT_PREALLOCATION_SIZE_LIMIT / PrimitiveTy::U64.size() + 1;
        let mut encoded_len = Vec::new();
        <LengthEncoding as SeqLen<DefaultConfig>>::write(&mut encoded_len, len).unwrap();

        let vector_error = Ty::Vec {
            ty: PrimitiveTy::U64,
        }
        .parse(Cursor::new(encoded_len))
        .unwrap_err();
        assert!(matches!(
            vector_error,
            wincode::ReadError::PreallocationSizeLimit { needed, limit }
                if needed == len * PrimitiveTy::U64.size()
                    && limit == DEFAULT_PREALLOCATION_SIZE_LIMIT
        ));

        let array_error = Ty::Array {
            ty: PrimitiveTy::U64,
            len,
        }
        .parse(Cursor::new(Vec::<u8>::new()))
        .unwrap_err();
        assert!(matches!(
            array_error,
            wincode::ReadError::PreallocationSizeLimit { needed, limit }
                if needed == len * PrimitiveTy::U64.size()
                    && limit == DEFAULT_PREALLOCATION_SIZE_LIMIT
        ));
    }

    #[test]
    fn dynamic_lazy_decode_enforces_its_allocation_limit() {
        use wincode::config::DEFAULT_PREALLOCATION_SIZE_LIMIT;

        let len = DEFAULT_PREALLOCATION_SIZE_LIMIT / size_of::<PrimitiveValue>() + 1;
        // SAFETY: `PrimitiveTy::U8` has an element width of one byte, so the
        // payload contains exactly `len` elements.
        let values =
            unsafe { LazyVec::new_unchecked(PrimitiveTy::U8, len, Cow::Owned(vec![0; len])) };

        let error = values.into_dyn_vec().unwrap_err();
        assert!(matches!(
            error,
            wincode::ReadError::PreallocationSizeLimit { needed, limit }
                if needed == len * size_of::<PrimitiveValue>()
                    && limit == DEFAULT_PREALLOCATION_SIZE_LIMIT
        ));
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

        let decoder = Decoder::new(U8EnumMessage::schema());

        let ping = wincode::serialize(&U8EnumMessage::Ping).unwrap();
        assert_eq!(decoder.fields(ping.as_slice()).unwrap().count(), 0);

        let value = wincode::serialize(&U8EnumMessage::Value(42)).unwrap();
        let fields = decoder
            .fields(value.as_slice())
            .unwrap()
            .collect::<ReadResult<Vec<_>>>()
            .unwrap();
        assert_eq!(fields.len(), 1);
        assert_field(
            &fields[0],
            "0",
            Ty::PrimitiveTy(PrimitiveTy::U64),
            Some(8),
            &Value::U64(42),
        );
    }

    #[test]
    fn enum_roundtrips_every_variant_shape() {
        let decoder = Decoder::new(EnumMessage::schema());

        assert_enum_message(&decoder, &EnumMessage::Ping, Vec::new());
        assert_enum_message(
            &decoder,
            &EnumMessage::Coordinates(42, true),
            vec![
                (
                    "0",
                    Ty::PrimitiveTy(PrimitiveTy::U64),
                    Some(8),
                    Value::U64(42),
                ),
                (
                    "1",
                    Ty::PrimitiveTy(PrimitiveTy::Bool),
                    Some(1),
                    Value::Bool(true),
                ),
            ],
        );
        assert_enum_message(
            &decoder,
            &EnumMessage::Payload {
                text: "hello".into(),
                bytes: vec![1, 2, 3, 4],
            },
            vec![
                (
                    "text",
                    Ty::String,
                    None,
                    Value::String(Cow::Owned("hello".into())),
                ),
                (
                    "bytes",
                    Ty::Vec {
                        ty: PrimitiveTy::U8,
                    },
                    None,
                    Value::Bytes(Cow::Owned(vec![1, 2, 3, 4])),
                ),
            ],
        );
    }

    #[test]
    fn enum_rejects_invalid_discriminant() {
        let decoder = Decoder::new(EnumMessage::schema());
        let payload = wincode::serialize(&u32::MAX).unwrap();

        let error = match decoder.fields(payload.as_slice()) {
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
        let decoder = Decoder::new(EnumMessage::schema());

        let truncated_discriminant = [0u8; 3];
        assert!(decoder.fields(&truncated_discriminant[..]).is_err());

        let mut truncated = wincode::serialize(&EnumMessage::Coordinates(42, true)).unwrap();
        truncated.pop();
        let truncated_result = decoder
            .fields(truncated.as_slice())
            .unwrap()
            .collect::<ReadResult<Vec<_>>>();
        assert!(truncated_result.is_err());

        let mut malformed = wincode::serialize(&EnumMessage::Coordinates(42, true)).unwrap();
        *malformed.last_mut().unwrap() = 2;
        let malformed_result = decoder
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
        let decoder = Decoder::new(Borrowable::schema());
        let fields = decoder
            .fields(payload.as_slice())
            .unwrap()
            .collect::<ReadResult<Vec<_>>>()
            .unwrap();

        assert_eq!(fields[0].name(), "text");
        assert_eq!(fields[1].name(), "bytes");
        match (fields[0].value(), fields[1].value()) {
            (Value::String(Cow::Borrowed(text)), Value::Bytes(Cow::Borrowed(bytes))) => {
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

        let decoder = Decoder::new(Generic::<u64>::schema());
        let payload = wincode::serialize(&Generic::Item(77u64)).unwrap();
        let fields = decoder
            .fields(payload.as_slice())
            .unwrap()
            .collect::<ReadResult<Vec<_>>>()
            .unwrap();

        assert_eq!(fields.len(), 1);
        assert_field(
            &fields[0],
            "0",
            Ty::PrimitiveTy(PrimitiveTy::U64),
            Some(8),
            &Value::U64(77),
        );

        let empty_payload = wincode::serialize(&Generic::<u64>::Empty).unwrap();
        assert_eq!(decoder.fields(empty_payload.as_slice()).unwrap().count(), 0);
    }

    proptest! {
        #[test]
        fn arbitrary_struct_fields_match(message in any::<StructMessage>()) {
            let payload = wincode::serialize(&message).unwrap();
            let decoder = Decoder::new(StructMessage::schema());
            let fields = decoder
                .fields(payload.as_slice())
                .unwrap()
                .collect::<ReadResult<Vec<_>>>()
                .unwrap();
            let mut fields = fields.into_iter().map(|field| field.value);

            prop_assert_eq!(fields.next(), Some(Value::U64(message.a)));
            prop_assert_eq!(fields.next(), Some(Value::Bool(message.b)));

            let Some(Value::Vec(values)) = fields.next() else {
                return Err(TestCaseError::fail("expected vals to be a lazy vector"));
            };
            prop_assert_eq!(values.len(), message.vals.len());
            prop_assert_eq!(values.ty(), PrimitiveTy::U64);
            let values = values
                .try_into_iter_as::<u64>()
                .unwrap()
                .collect::<ReadResult<Vec<_>>>()
                .unwrap();
            prop_assert_eq!(values.as_slice(), message.vals.as_slice());

            prop_assert_eq!(
                fields.next(),
                Some(Value::String(Cow::Borrowed(message.str.as_str())))
            );
            prop_assert_eq!(
                fields.next(),
                Some(Value::Bytes(Cow::Borrowed(message.bytes.as_slice())))
            );

            let Some(Value::Vec(values)) = fields.next() else {
                return Err(TestCaseError::fail("expected ar to be a lazy vector"));
            };
            prop_assert_eq!(
                values
                    .try_into_iter_as::<u64>()
                    .unwrap()
                    .collect::<ReadResult<Vec<_>>>()
                    .unwrap(),
                message.ar
            );
            prop_assert_eq!(
                fields.next(),
                Some(Value::Bytes(Cow::Borrowed(message.ar_bytes.as_slice())))
            );
            prop_assert!(fields.next().is_none());
        }

        #[test]
        fn arbitrary_enum_fields_match(message in any::<EnumMessage>()) {
            let payload = wincode::serialize(&message).unwrap();
            let decoder = Decoder::new(EnumMessage::schema());
            let actual = decoder
                .fields(payload.as_slice())
                .unwrap()
                .collect::<ReadResult<Vec<_>>>()
                .unwrap()
                .into_iter()
                .map(|field| field.value)
                .collect::<Vec<_>>();
            let expected = match &message {
                EnumMessage::Ping => Vec::new(),
                EnumMessage::Coordinates(x, y) => vec![Value::U64(*x), Value::Bool(*y)],
                EnumMessage::Payload { text, bytes } => vec![
                    Value::String(Cow::Borrowed(text.as_str())),
                    Value::Bytes(Cow::Borrowed(bytes.as_slice())),
                ],
            };

            prop_assert_eq!(actual, expected);
        }

        #[test]
        fn arbitrary_truncated_struct_is_rejected(
            message in any::<StructMessage>(),
            cut_seed in any::<usize>(),
        ) {
            let mut payload = wincode::serialize(&message).unwrap();
            let cut = cut_seed % payload.len();
            payload.truncate(cut);

            let decoder = Decoder::new(StructMessage::schema());
            let result = decoder
                .fields(payload.as_slice())
                .and_then(|fields| fields.collect::<ReadResult<Vec<_>>>());
            prop_assert!(result.is_err());
        }
    }

    macro_rules! primitive_vector_property {
        ($name:ident, $ty:ty, $variant:path, $strategy:expr) => {
            proptest! {
                #[test]
                fn $name(values in proptest::collection::vec($strategy, 0..64)) {
                    #[derive(SchemaDynamic, SchemaRead, SchemaWrite)]
                    #[wincode_dynamic(internal)]
                    struct Message {
                        values: Vec<$ty>,
                    }

                    let message = Message {
                        values: values.clone(),
                    };
                    let payload = wincode::serialize(&message).unwrap();
                    let decoder = Decoder::new(Message::schema());
                    let value = decoder
                        .fields(payload.as_slice())
                        .unwrap()
                        .next()
                        .unwrap()
                        .unwrap();
                    prop_assert_eq!(value.name(), "values");
                    prop_assert!(
                        matches!(value.ty(), Ty::Vec { .. }),
                        "field type was not a vector"
                    );
                    prop_assert_eq!(value.size(), None);
                    let Value::Vec(lazy) = value.value else {
                        return Err(TestCaseError::fail("expected a lazy vector"));
                    };

                    prop_assert_eq!(lazy.len(), values.len());
                    prop_assert_eq!(lazy.is_empty(), values.is_empty());
                    prop_assert!(lazy.has_borrowed_payload());
                    prop_assert_eq!(
                        lazy.clone().into_dyn_vec().unwrap(),
                        values.iter().copied().map($variant).collect::<Vec<_>>()
                    );
                    prop_assert_eq!(
                        lazy
                            .try_into_iter_as::<$ty>()
                            .unwrap()
                            .collect::<ReadResult<Vec<_>>>()
                            .unwrap(),
                        values
                    );
                }
            }
        };
    }

    primitive_vector_property!(arbitrary_u16_vector, u16, PrimitiveValue::U16, any::<u16>());
    primitive_vector_property!(arbitrary_u32_vector, u32, PrimitiveValue::U32, any::<u32>());
    primitive_vector_property!(arbitrary_u64_vector, u64, PrimitiveValue::U64, any::<u64>());
    primitive_vector_property!(arbitrary_i8_vector, i8, PrimitiveValue::I8, any::<i8>());
    primitive_vector_property!(arbitrary_i16_vector, i16, PrimitiveValue::I16, any::<i16>());
    primitive_vector_property!(arbitrary_i32_vector, i32, PrimitiveValue::I32, any::<i32>());
    primitive_vector_property!(arbitrary_i64_vector, i64, PrimitiveValue::I64, any::<i64>());
    primitive_vector_property!(
        arbitrary_bool_vector,
        bool,
        PrimitiveValue::Bool,
        any::<bool>()
    );
    primitive_vector_property!(
        arbitrary_f32_vector,
        f32,
        PrimitiveValue::F32,
        -1.0e20f32..1.0e20f32
    );
    primitive_vector_property!(
        arbitrary_f64_vector,
        f64,
        PrimitiveValue::F64,
        -1.0e100f64..1.0e100f64
    );
}
