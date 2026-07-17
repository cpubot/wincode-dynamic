# wincode-dynamic

Runtime schemas and reflective decoding for [`wincode`](https://crates.io/crates/wincode).

`wincode` deserializes bytes into a concrete Rust type known at compile time.
`wincode-dynamic` is for when you _don't_ have that type at compile time.
It decodes the same wincode wire format against a schema value supplied at runtime. Because a schema is itself
`wincode`-serializable, the usual pattern is to send it as the first payload on a
connection and then stream values that a peer can
reflect / iterate over without compile-time knowledge of the schema. Decoding is lazy and borrows from
the input.

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

// Producer: announce the schema once, up front...
let schema = wincode::serialize(&Account::schema())?;
// ...then stream values encoded with plain `wincode`.
let record = wincode::serialize(&Account {
    lamports: 42,
    owner: [7; 32],
    executable: true,
})?;

// Consumer: never had the `Account` type at compile time. Read the schema off the wire...
let decoder = Decoder::new(wincode::deserialize::<RootSchema>(&schema)?);

// ...and reflect over every record that follows.
for field in decoder.fields(&record[..])? {
    let field = field?;
    println!("{} = {:?}", field.name(), field.value());
}
// lamports = U64(42)
// owner = Bytes([7, 7, 7, ...])
// executable = Bool(true)
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
