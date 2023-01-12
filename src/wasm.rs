#![cfg(target_arch = "wasm32")]

use wasm_bindgen;

use crate::repl::Repl;
use crate::repl::ReplError;

use cfg_if::cfg_if;
use wasm_bindgen::prelude::*;

cfg_if! {
    // When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
    // allocator.
    if #[cfg(feature = "wee_alloc")] {
        extern crate wee_alloc;
        #[global_allocator]
        static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
    }
}

#[wasm_bindgen(module = "/js/damasc.js")]
extern "C" {
    fn show_error(stmt: &str, error: &str);
    fn show_result(stmt: &str, result: &str);
}

#[wasm_bindgen]
pub struct WasmRepl {
    state: Box<Repl<'static,'static,'static, 'static>>,
}


#[wasm_bindgen]
impl WasmRepl {

    #[wasm_bindgen(constructor)]
    pub fn default() -> Self {
        Self {
            state: Box::new(Repl::new("init")),
        }
    }


    #[wasm_bindgen]
    pub fn eval(&mut self, input:&str) {
        let stmt = match crate::parser::statement(input) {
            Ok((_, s)) => s,
            Err(e) => {
                return show_error(input, &format!("read error: {e}"));
            }
        };

        match self.state.execute(stmt) {
            Ok(r) => {
                return show_result(input, &format!("{r}"))
            }
            Err(ReplError::Exit) => {},
            Err(e) => return show_error(input, &format!("Error: {e:?}")),
        }
    }
}

