#[cfg(test)]
mod tests {
    use crate::error_ctx;
    use crate::system::errors::context::IErrorContext;

    #[test]
    fn test_empty_context() {
        let ctx = error_ctx! {};
        assert!(ctx.to_vec().is_none() || ctx.to_vec().unwrap().is_empty());
    }

    #[test]
    fn test_single_literal_entry() {
        let ctx = error_ctx! {
            "operation" => "test_operation"
        };

        #[cfg(not(target_arch = "riscv32"))]
        {
            let elements = ctx.to_vec().unwrap();
            assert_eq!(elements.len(), 1);
            assert_eq!(elements[0].name, "operation");
            assert_eq!(elements[0].value, "test_operation");
        }
    }

    #[test]
    fn test_multiple_literal_entries() {
        let ctx = error_ctx! {
            "operation" => "test_op",
            "status" => "failed",
            "code" => 42
        };

        #[cfg(not(target_arch = "riscv32"))]
        {
            let elements = ctx.to_vec().unwrap();
            assert_eq!(elements.len(), 3);

            assert_eq!(elements[0].name, "operation");
            assert_eq!(elements[0].value, "test_op");

            assert_eq!(elements[1].name, "status");
            assert_eq!(elements[1].value, "failed");

            assert_eq!(elements[2].name, "code");
            assert_eq!(elements[2].value, "42");
        }
    }

    #[test]
    fn test_identifier_shortcut() {
        let operation = "test_operation";
        let code = 404;

        let ctx = error_ctx! {
            operation,
            code
        };

        #[cfg(not(target_arch = "riscv32"))]
        {
            let elements = ctx.to_vec().unwrap();
            assert_eq!(elements.len(), 2);

            assert_eq!(elements[0].name, "operation");
            assert_eq!(elements[0].value, "test_operation");

            assert_eq!(elements[1].name, "code");
            assert_eq!(elements[1].value, "404");
        }
    }

    #[test]
    fn test_detailed_entries() {
        let ctx = error_ctx! {
            #[detailed] "debug_info" => "sensitive_data",
            "public_info" => "safe_data"
        };

        #[cfg(not(target_arch = "riscv32"))]
        {
            let elements = ctx.to_vec().unwrap();

            // Without detailed_errors feature, only public_info should be present
            #[cfg(not(feature = "detailed_errors"))]
            {
                assert_eq!(elements.len(), 1);
                assert_eq!(elements[0].name, "public_info");
                assert_eq!(elements[0].value, "safe_data");
            }

            // With detailed_errors feature, both should be present
            #[cfg(feature = "detailed_errors")]
            {
                assert_eq!(elements.len(), 2);
                assert_eq!(elements[0].name, "debug_info");
                assert_eq!(elements[0].value, "sensitive_data");
                assert_eq!(elements[1].name, "public_info");
                assert_eq!(elements[1].value, "safe_data");
            }
        }
    }

    #[test]
    fn test_detailed_identifier_shortcut() {
        let sensitive_var = "secret";
        let public_var = "public";

        let ctx = error_ctx! {
            #[detailed] sensitive_var,
            public_var
        };

        #[cfg(not(target_arch = "riscv32"))]
        {
            let elements = ctx.to_vec().unwrap();

            #[cfg(not(feature = "detailed_errors"))]
            {
                assert_eq!(elements.len(), 1);
                assert_eq!(elements[0].name, "public_var");
                assert_eq!(elements[0].value, "public");
            }

            #[cfg(feature = "detailed_errors")]
            {
                assert_eq!(elements.len(), 2);
                assert_eq!(elements[0].name, "sensitive_var");
                assert_eq!(elements[0].value, "secret");
                assert_eq!(elements[1].name, "public_var");
                assert_eq!(elements[1].value, "public");
            }
        }
    }

    #[test]
    fn test_mixed_entry_types() {
        let var = "variable_value";

        let ctx = error_ctx! {
            "literal" => "literal_value",
            var,
            "number" => 123,
            #[detailed] "debug" => "debug_value",
            #[detailed] var
        };

        #[cfg(not(target_arch = "riscv32"))]
        {
            let elements = ctx.to_vec().unwrap();

            #[cfg(not(feature = "detailed_errors"))]
            {
                // Should have: literal, var, number
                assert_eq!(elements.len(), 3);
                assert_eq!(elements[0].name, "literal");
                assert_eq!(elements[0].value, "literal_value");
                assert_eq!(elements[1].name, "var");
                assert_eq!(elements[1].value, "variable_value");
                assert_eq!(elements[2].name, "number");
                assert_eq!(elements[2].value, "123");
            }

            #[cfg(feature = "detailed_errors")]
            {
                // Should have all 5 entries
                assert_eq!(elements.len(), 5);
                assert_eq!(elements[0].name, "literal");
                assert_eq!(elements[0].value, "literal_value");
                assert_eq!(elements[1].name, "var");
                assert_eq!(elements[1].value, "variable_value");
                assert_eq!(elements[2].name, "number");
                assert_eq!(elements[2].value, "123");
                assert_eq!(elements[3].name, "debug");
                assert_eq!(elements[3].value, "debug_value");
                assert_eq!(elements[4].name, "var");
                assert_eq!(elements[4].value, "variable_value");
            }
        }
    }

    #[test]
    fn test_debug_format_function() {
        #[derive(Debug)]
        struct TestStruct {
            #[allow(dead_code)]
            field: i32,
        }

        let test_val = TestStruct { field: 42 };

        let ctx = error_ctx! {
            "formatted" => debug_format(test_val)
        };

        #[cfg(not(target_arch = "riscv32"))]
        {
            let elements = ctx.to_vec().unwrap();
            assert_eq!(elements.len(), 1);
            assert_eq!(elements[0].name, "formatted");
            assert!(elements[0].value.contains("TestStruct"));
            assert!(elements[0].value.contains("field: 42"));
        }
    }

    #[test]
    fn test_trailing_commas() {
        let ctx = error_ctx! {
            "test" => "value",
        };

        #[cfg(not(target_arch = "riscv32"))]
        {
            let elements = ctx.to_vec().unwrap();
            assert_eq!(elements.len(), 1);
            assert_eq!(elements[0].name, "test");
            assert_eq!(elements[0].value, "value");
        }
    }

    #[test]
    fn test_multiple_trailing_commas() {
        let ctx = error_ctx! {
            "test" => "value",,,
        };

        #[cfg(not(target_arch = "riscv32"))]
        {
            let elements = ctx.to_vec().unwrap();
            assert_eq!(elements.len(), 1);
            assert_eq!(elements[0].name, "test");
            assert_eq!(elements[0].value, "value");
        }
    }

    #[test]
    fn test_context_get_method() {
        let ctx = error_ctx! {
            "key1" => "value1",
            "key2" => "value2"
        };

        #[cfg(not(target_arch = "riscv32"))]
        {
            assert_eq!(ctx.get("key1"), Some(&"value1".to_string()));
            assert_eq!(ctx.get("key2"), Some(&"value2".to_string()));
            assert_eq!(ctx.get("nonexistent"), None);
        }

        #[cfg(target_arch = "riscv32")]
        {
            // Empty context always returns None
            assert_eq!(ctx.get("key1"), None);
            assert_eq!(ctx.get("key2"), None);
        }
    }

    #[test]
    fn test_context_display() {
        let ctx = error_ctx! {
            "operation" => "test",
            "status" => "failed"
        };

        let display_str = format!("{}", ctx);

        #[cfg(not(target_arch = "riscv32"))]
        {
            assert!(display_str.contains("operation => test"));
            assert!(display_str.contains("status => failed"));
        }

        #[cfg(target_arch = "riscv32")]
        {
            // Empty context displays as empty
            assert_eq!(display_str, "");
        }
    }

    #[test]
    fn test_complex_mixed_usage() {
        let operation = "complex_operation";
        let error_code = 500;
        let debug_info = "detailed debugging information";

        let ctx = error_ctx! {
            "stage" => "processing",
            operation,
            "code" => error_code,
            #[detailed] "debug_info" => debug_info,
            #[detailed] error_code,
            "timestamp" => debug_format(std::time::SystemTime::now()),
        };

        #[cfg(not(target_arch = "riscv32"))]
        {
            let elements = ctx.to_vec().unwrap();

            #[cfg(not(feature = "detailed_errors"))]
            {
                // Should have: stage, operation, code, timestamp
                assert_eq!(elements.len(), 4);
                assert_eq!(elements[0].name, "stage");
                assert_eq!(elements[0].value, "processing");
                assert_eq!(elements[1].name, "operation");
                assert_eq!(elements[1].value, "complex_operation");
                assert_eq!(elements[2].name, "code");
                assert_eq!(elements[2].value, "500");
                assert_eq!(elements[3].name, "timestamp");
                assert!(elements[3].value.contains("SystemTime"));
            }

            #[cfg(feature = "detailed_errors")]
            {
                // Should have all 6 entries
                assert_eq!(elements.len(), 6);
                assert_eq!(elements[0].name, "stage");
                assert_eq!(elements[0].value, "processing");
                assert_eq!(elements[1].name, "operation");
                assert_eq!(elements[1].value, "complex_operation");
                assert_eq!(elements[2].name, "code");
                assert_eq!(elements[2].value, "500");
                assert_eq!(elements[3].name, "debug_info");
                assert_eq!(elements[3].value, "detailed debugging information");
                assert_eq!(elements[4].name, "error_code");
                assert_eq!(elements[4].value, "500");
                assert_eq!(elements[5].name, "timestamp");
                assert!(elements[5].value.contains("SystemTime"));
            }
        }
    }

    #[test]
    fn test_numeric_and_boolean_values() {
        let enabled = true;
        let count = 42u64;
        let ratio = 3.14f64;

        let ctx = error_ctx! {
            "enabled" => enabled,
            "count" => count,
            "ratio" => ratio,
            count,
            enabled
        };

        #[cfg(not(target_arch = "riscv32"))]
        {
            let elements = ctx.to_vec().unwrap();
            assert_eq!(elements.len(), 5);

            assert_eq!(elements[0].name, "enabled");
            assert_eq!(elements[0].value, "true");

            assert_eq!(elements[1].name, "count");
            assert_eq!(elements[1].value, "42");

            assert_eq!(elements[2].name, "ratio");
            assert_eq!(elements[2].value, "3.14");

            assert_eq!(elements[3].name, "count");
            assert_eq!(elements[3].value, "42");

            assert_eq!(elements[4].name, "enabled");
            assert_eq!(elements[4].value, "true");
        }
    }

    #[test]
    fn test_error_context_trait_methods() {
        let ctx = error_ctx! {
            "key1" => "value1",
            "key2" => "value2",
            "key3" => "value3"
        };

        #[cfg(not(target_arch = "riscv32"))]
        {
            // Test get method
            assert_eq!(ctx.get("key1"), Some(&"value1".to_string()));
            assert_eq!(ctx.get("key2"), Some(&"value2".to_string()));
            assert_eq!(ctx.get("key3"), Some(&"value3".to_string()));
            assert_eq!(ctx.get("nonexistent"), None);

            // Test to_vec
            let vec1 = ctx.to_vec().unwrap();
            assert_eq!(vec1.len(), 3);

            // Test into_vec
            let vec2 = ctx.into_vec().unwrap();
            assert_eq!(vec2.len(), 3);
            assert_eq!(vec1, vec2);
        }

        #[cfg(target_arch = "riscv32")]
        {
            assert_eq!(ctx.get("key1"), None);
            assert_eq!(ctx.to_vec(), None);
            assert_eq!(ctx.into_vec(), None);
        }
    }

    #[test]
    fn test_nested_debug_format() {
        #[derive(Debug)]
        struct InnerStruct {
            #[allow(dead_code)]
            id: u32,
            #[allow(dead_code)]
            name: String,
        }

        #[derive(Debug)]
        struct OuterStruct {
            #[allow(dead_code)]
            inner: InnerStruct,
            #[allow(dead_code)]
            active: bool,
        }

        let data = OuterStruct {
            inner: InnerStruct {
                id: 123,
                name: "test".to_string(),
            },
            active: true,
        };

        let ctx = error_ctx! {
            "data" => debug_format(data),
            "simple" => "plain_text"
        };

        #[cfg(not(target_arch = "riscv32"))]
        {
            let elements = ctx.to_vec().unwrap();
            assert_eq!(elements.len(), 2);

            assert_eq!(elements[0].name, "data");
            assert!(elements[0].value.contains("OuterStruct"));
            assert!(elements[0].value.contains("InnerStruct"));
            assert!(elements[0].value.contains("123"));
            assert!(elements[0].value.contains("test"));
            assert!(elements[0].value.contains("true"));

            assert_eq!(elements[1].name, "simple");
            assert_eq!(elements[1].value, "plain_text");
        }
    }

    #[test]
    fn test_empty_context_methods() {
        let ctx = error_ctx! {};

        // Test that empty context behaves correctly
        #[cfg(not(target_arch = "riscv32"))]
        {
            let vec = ctx.to_vec().unwrap();
            assert_eq!(vec.len(), 0);

            assert_eq!(ctx.get("any_key"), None);

            let display_str = format!("{}", ctx);
            assert_eq!(display_str, "");
        }

        #[cfg(target_arch = "riscv32")]
        {
            assert_eq!(ctx.to_vec(), None);
            assert_eq!(ctx.get("any_key"), None);

            let display_str = format!("{}", ctx);
            assert_eq!(display_str, "");
        }
    }

    #[test]
    fn test_lazy_detailed_evaluation() {
        use core::sync::atomic::{AtomicUsize, Ordering};

        static DETAILED_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
        static NORMAL_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

        // Reset counters
        DETAILED_CALL_COUNT.store(0, Ordering::Relaxed);
        NORMAL_CALL_COUNT.store(0, Ordering::Relaxed);

        let _ctx = error_ctx! {
            "normal" => {
                NORMAL_CALL_COUNT.fetch_add(1, Ordering::Relaxed);
                "normal_value"
            },
            #[detailed] "detailed" => {
                DETAILED_CALL_COUNT.fetch_add(1, Ordering::Relaxed);
                "detailed_value"
            },
        };

        // Normal expressions should always be evaluated (on non-RISC-V)
        #[cfg(not(target_arch = "riscv32"))]
        assert_eq!(NORMAL_CALL_COUNT.load(Ordering::Relaxed), 1);

        #[cfg(target_arch = "riscv32")]
        assert_eq!(NORMAL_CALL_COUNT.load(Ordering::Relaxed), 0); // Nothing evaluated on RISC-V

        // Detailed expressions should only be evaluated when detailed_errors is enabled
        #[cfg(all(not(target_arch = "riscv32"), feature = "detailed_errors"))]
        assert_eq!(DETAILED_CALL_COUNT.load(Ordering::Relaxed), 1);

        #[cfg(not(all(not(target_arch = "riscv32"), feature = "detailed_errors")))]
        assert_eq!(DETAILED_CALL_COUNT.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_lazy_expensive_computation() {
        use core::sync::atomic::{AtomicUsize, Ordering};

        static EXPENSIVE_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

        fn very_expensive_computation() -> String {
            EXPENSIVE_CALL_COUNT.fetch_add(1, Ordering::Relaxed);
            // Simulate expensive work
            "expensive_result".to_string()
        }

        // Reset counter
        EXPENSIVE_CALL_COUNT.store(0, Ordering::Relaxed);

        let _ctx = error_ctx! {
            "cheap" => "simple_value",
            #[detailed] "expensive" => very_expensive_computation(),
        };

        // Verify that expensive computation is only called when detailed_errors is enabled
        #[cfg(all(not(target_arch = "riscv32"), feature = "detailed_errors"))]
        assert_eq!(EXPENSIVE_CALL_COUNT.load(Ordering::Relaxed), 1);

        #[cfg(not(all(not(target_arch = "riscv32"), feature = "detailed_errors")))]
        assert_eq!(EXPENSIVE_CALL_COUNT.load(Ordering::Relaxed), 0);

        #[cfg(not(target_arch = "riscv32"))]
        {
            let elements = _ctx.to_vec().unwrap();

            #[cfg(feature = "detailed_errors")]
            {
                assert_eq!(elements.len(), 2);
                assert_eq!(elements[1].name, "expensive");
                assert_eq!(elements[1].value, "expensive_result");
            }

            #[cfg(not(feature = "detailed_errors"))]
            {
                // Only the cheap computation should be present
                assert_eq!(elements.len(), 1);
                assert_eq!(elements[0].name, "cheap");
                assert_eq!(elements[0].value, "simple_value");
            }
        }
    }

    #[test]
    fn test_push_lazy_method_directly() {
        use crate::system::errors::context::IErrorContext;
        use core::sync::atomic::{AtomicUsize, Ordering};

        static CLOSURE_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

        // Reset counter
        CLOSURE_CALL_COUNT.store(0, Ordering::Relaxed);

        let ctx = crate::system::errors::context::ErrorContext::default();

        let ctx = ctx.push_lazy(
            "test",
            || {
                CLOSURE_CALL_COUNT.fetch_add(1, Ordering::Relaxed);
                "lazy_value".to_string()
            },
            crate::system::errors::context::element::ValueVisibility::DetailedOnly,
        );

        // Verify the closure was called appropriately based on feature flags
        #[cfg(all(not(target_arch = "riscv32"), feature = "detailed_errors"))]
        assert_eq!(CLOSURE_CALL_COUNT.load(Ordering::Relaxed), 1);

        #[cfg(not(all(not(target_arch = "riscv32"), feature = "detailed_errors")))]
        assert_eq!(CLOSURE_CALL_COUNT.load(Ordering::Relaxed), 0);

        #[cfg(not(target_arch = "riscv32"))]
        {
            let elements = ctx.to_vec().unwrap();

            #[cfg(feature = "detailed_errors")]
            {
                assert_eq!(elements.len(), 1);
                assert_eq!(elements[0].name, "test");
                assert_eq!(elements[0].value, "lazy_value");
            }

            #[cfg(not(feature = "detailed_errors"))]
            {
                assert_eq!(elements.len(), 0);
            }
        }
    }
}
