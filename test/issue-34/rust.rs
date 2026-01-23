#[tsync]
struct SomeStruct {
    #[serde(skip)]
    foo: ThisIsARandomExternalType,
    #[tsync(ts_type = "any")]
    foo2: ThisIsARandomExternalType,
}
