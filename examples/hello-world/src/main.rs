use wasmide::prelude::*;

fn main() {
    Component::root(Style("container mx-auto bg-blue-200 grid place-items-center"))
        .with(html::p(
            Value("Hello, World!"),
            Style("text-9xl"),
        ));
}
