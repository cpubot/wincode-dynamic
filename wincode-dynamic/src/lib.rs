use wincode::{ReadResult, SchemaRead, SchemaWrite, io::Reader};

mod ty;
mod value;
mod wincode_extra;
pub use {ty::*, value::*, wincode_dynamic_derive::*};

#[derive(SchemaRead, SchemaWrite, Debug, Clone)]
pub struct Field {
    name: String,
    ty: Ty,
}

impl Field {
    pub fn new(name: impl Into<String>, ty: Ty) -> Self {
        Self {
            name: name.into(),
            ty,
        }
    }

    #[inline]
    pub fn parse<'de>(&self, reader: impl Reader<'de>) -> ReadResult<Value<'de>> {
        self.ty.parse(reader)
    }
}

#[derive(SchemaRead, SchemaWrite, Debug, Clone)]
pub struct Header {
    name: String,
    fields: Box<[Field]>,
}

impl Header {
    pub fn new(name: impl Into<String>, fields: Box<[Field]>) -> Self {
        Self {
            name: name.into(),
            fields,
        }
    }
}

pub trait SchemaDynamic {
    fn schema() -> Header;
}

pub struct SchemaRuntime {
    header: Header,
}

impl SchemaRuntime {
    pub fn new(header: Header) -> Self {
        Self { header }
    }

    pub fn name(&self) -> &str {
        &self.header.name
    }

    #[inline]
    pub fn fields<'a, 'de>(
        &'a self,
        mut reader: impl Reader<'de> + 'a,
    ) -> impl Iterator<Item = ReadResult<Value<'de>>> + 'a {
        self.header
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
