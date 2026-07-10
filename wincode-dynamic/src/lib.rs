use wincode::{ReadResult, SchemaRead, SchemaWrite, io::Reader};

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
    fn schema() -> Schema;
}

pub struct SchemaRuntime {
    schema: Schema,
}

impl SchemaRuntime {
    pub fn new(schema: Schema) -> Self {
        Self { schema }
    }

    pub fn name(&self) -> &str {
        &self.schema.name
    }

    pub fn size(&self) -> Option<usize> {
        self.schema.size
    }

    #[inline]
    pub fn fields<'a, 'de>(
        &'a self,
        mut reader: impl Reader<'de> + 'a,
    ) -> impl Iterator<Item = ReadResult<Value<'de>>> + 'a {
        self.schema
            .fields
            .iter()
            .map(move |field| field.parse(reader.by_ref()))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn example() {
        #[derive(SchemaDynamic, SchemaRead, SchemaWrite, PartialEq, Debug)]
        #[wincode_dynamic(internal)]
        struct Hello {
            a: u64,
            b: bool,
            vals: Vec<u64>,
            str: String,
            bytes: Vec<u8>,
            ar: [u64; 4],
            ar_bytes: [u8; 8],
        }

        let hello = Hello {
            a: 42,
            b: true,
            vals: vec![333; 4],
            str: String::from("hello world"),
            bytes: vec![42; 8],
            ar: [444; 4],
            ar_bytes: [43; 8],
        };

        let schema = Hello::schema();
        let dyn_parser = SchemaRuntime::new(schema);

        let payload = wincode::serialize(&hello).unwrap();
        let result = dyn_parser
            .fields(&payload[..])
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
}
