fn main() {
    tonic_build::configure()
        .compile_protos(&["src/messages.proto"], &["src/"])
        .unwrap();
}
