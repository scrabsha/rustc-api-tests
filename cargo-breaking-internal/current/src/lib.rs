#![allow(unused)]

pub use std::string::String;

pub mod foo {
    pub fn bar() {}
}

pub fn f(a: u8) {
    panic!("foobar")
}

pub trait T {
    fn a() -> u32 {
        42
    }
}

extern "C" {
    fn my_c_function(x: i32) -> bool;
}
