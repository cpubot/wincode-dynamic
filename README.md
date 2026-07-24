# wincode-dynamic

Runtime schemas and reflective decoding for [`wincode`](https://crates.io/crates/wincode).

`wincode` deserializes bytes into a concrete Rust type known at compile time.
`wincode-dynamic` is for when you _don't_ have that type at compile time.
It decodes the same wincode wire format against a schema value supplied at runtime. Because a schema is itself
`wincode`-serializable, the usual pattern is to send it as the first payload on a
connection and then stream values that a peer can
reflect / iterate over without compile-time knowledge of the schema. Primitive
values are decoded by value. Strings and the encoded payloads of primitive
arrays and vectors borrow from the input when the reader supports stable
borrowing; non-byte arrays and vectors are decoded lazily as they are iterated
without allocating.

This crate is [no_std](https://docs.rust-embedded.org/book/intro/no-std.html).

## Supported field types

Runtime schemas currently support:

- primitives: `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`, `f32`,
  `f64`, and `bool`;
- strings; and
- arrays and vectors of primitives.

Producers can serialize a slice instead of allocating a `Vec`, or a `&str`
instead of allocating a `String`; each uses the same wire format as its owned
counterpart. The decoder returns `u8` sequences as raw bytes and other primitive
sequences as [`LazyVec`](https://docs.rs/wincode-dynamic/latest/wincode_dynamic/struct.LazyVec.html).
With a supported reader, this data borrows directly from the input.

Schema generation and dynamic decoding currently support only wincode's
`DefaultConfig`. Custom wincode configurations are not supported.

## Send the schema, then values

Derive a schema alongside the usual `wincode` traits. The producer announces it up
front; the consumer reads it off the wire and reflects over everything that follows.

```rust
use wincode::{SchemaRead, SchemaWrite};
use wincode_dynamic::{Decoder, RootSchema, SchemaDynamic};

#[derive(SchemaDynamic, SchemaRead, SchemaWrite)]
struct Account {
    lamports: u64,
    owner: [u8; 32],
    executable: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Producer: announce the schema once, up front...
    let schema = wincode::serialize(&Account::schema())?;
    // ...then stream values encoded with plain `wincode`.
    let record = wincode::serialize(&Account {
        lamports: 42,
        owner: [7; 32],
        executable: true,
    })?;

    // Consumer: never had the `Account` type at compile time.
    // Read the schema off the wire...
    let decoder = Decoder::new(wincode::deserialize::<RootSchema>(&schema)?);

    // ...and reflect over every record that follows.
    for field in decoder.fields(&record[..])? {
        let field = field?;
        println!("{} = {:?}", field.name(), field.value());
    }
    // lamports = U64(42)
    // owner = Bytes([7, 7, 7, ...])
    // executable = Bool(true)
    Ok(())
}
```

## Serialized size metadata

The derive reports a maximum serialized size when it can determine one from
wincode's static type metadata:

```rust
use wincode::{SchemaRead, SchemaWrite};
use wincode_dynamic::{SchemaDynamic, SerializedSize};

#[derive(SchemaDynamic, SchemaRead, SchemaWrite)]
struct Event {
    timestamp: u64,
    active: bool,
}

assert_eq!(Event::SERIALIZED_SIZE, SerializedSize::Static(9));
```

Types with dynamically sized fields report `SerializedSize::Dynamic(n)`, where
`n` is the largest contribution from fields whose sizes are statically known.
Callers can add an application-specific allowance for the remaining dynamic
data when sizing their storage or transport.

For fields using `#[wincode(with = ...)]`, serialized-size metadata comes from
the adapter, while the runtime schema continues to describe the Rust field's
`DynTy`. The adapter must preserve that wire representation when the schema is
used for dynamic decoding.
