#[allow(dead_code)]
pub(crate) mod colors {
    pub const RESET: &str = "\x1b[0m";

    pub const BLACK: &str = "\x1b[30m";
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
    pub const WHITE: &str = "\x1b[37m";

    pub const BRIGHT_BLACK: &str = "\x1b[90m";
    pub const BRIGHT_RED: &str = "\x1b[91m";
    pub const BRIGHT_GREEN: &str = "\x1b[92m";
    pub const BRIGHT_YELLOW: &str = "\x1b[93m";
    pub const BRIGHT_BLUE: &str = "\x1b[94m";
    pub const BRIGHT_MAGENTA: &str = "\x1b[95m";
    pub const BRIGHT_CYAN: &str = "\x1b[96m";
    pub const BRIGHT_WHITE: &str = "\x1b[97m";

    // Extended palette
    pub const ORANGE: &str = "\x1b[38;5;214m";
    pub const BRIGHT_ORANGE: &str = "\x1b[38;5;214m";
}

#[cfg(test)]
mod test {
    use std::io::{Read, Write};

    use wasmer::wat2wasm;

    #[ignore = "depends on some testsuite"]
    #[test]
    fn convert_wat() {
        let filename = "loop_with_params";
        let mut file = std::fs::File::open(format!("./{}.wat", filename)).unwrap();
        let mut buffer = vec![];
        file.read_to_end(&mut buffer).unwrap();
        let bytecode = wat2wasm(&buffer).unwrap();
        let mut file = std::fs::File::create(format!("./{}.wasm", filename)).unwrap();
        file.write_all(&bytecode.to_owned()).unwrap();
    }

    const _MODULE_WAT: &str = r#"
        (module
        (import "spectest" "memory" (memory 0))
        (data (i32.const 0) "a")
      )
    "#;

    #[test]
    fn instantiate_in_wasmer() {
        use wasmer::*;

        let mut store = Store::default();
        let module = Module::new(
            &store,
            "
    (module
      (type $sum_t (func (param i32 i32) (result i32)))
      (func $sum_f (type $sum_t) (param $x i32) (param $y i32) (result i32)
        local.get $x
        local.get $y
        i32.add)
      (export \"sum\" (func $sum_f)))
    ",
        )
        .map_err(|e| format!("{e:?}"))
        .unwrap();

        let imports = Imports::new();
        let instance = Instance::new(&mut store, &module, &imports)
            .map_err(|e| format!("{e:?}"))
            .unwrap();

        // The function is cloned to “break” the connection with `instance`.
        let sum = instance
            .exports
            .get_function("sum")
            .map_err(|e| format!("{e:?}"))
            .unwrap()
            .clone();

        drop(instance);

        // All instances have been dropped, but `sum` continues to work!
        assert_eq!(
            sum.call(&mut store, &[Value::I32(1), Value::I32(2)])
                .map_err(|e| format!("{e:?}"))
                .unwrap()
                .into_vec(),
            vec![Value::I32(3)],
        );
    }
}
