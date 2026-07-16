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

## Bound the serialized size

For a dynamically sized event that will be written into fixed-size storage,
declare a type-level serialized-size bound:

```rust
use wincode::{SchemaRead, SchemaWrite};
use wincode_dynamic::SchemaDynamic;

#[derive(SchemaDynamic, SchemaRead, SchemaWrite)]
#[wincode_dynamic(max_serialized_size = 1024)]
struct Event {
    message: String,
    values: Vec<u64>,
}

const EVENT_CELL_SIZE: usize = Event::MAX_SERIALIZED_SIZE;
assert_eq!(EVENT_CELL_SIZE, 1024);
```

The bound is a compile-time constant but does not restrict the runtime lengths
of `String` or `Vec` fields. Code writing an event into a cell must reject an
encoding larger than `MAX_SERIALIZED_SIZE`. Without an explicit bound, derives
use the exact size of statically sized types and
`UNBOUNDED_SERIALIZED_SIZE` for dynamically sized types.

Do not set `max_serialized_size` on a fixed-width struct or enum. Its maximum is
inferred automatically, and specifying the attribute is a compile-time error.
For an enum whose variants have different sizes, the inferred maximum is the
tag size plus the size of its largest variant.
