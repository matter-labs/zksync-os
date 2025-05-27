#[derive(Debug, Clone)]
pub enum StorageModelKind<A, B> {
    Old(A),
    New(B),
}
