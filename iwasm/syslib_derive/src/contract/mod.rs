use parsed::Contract;
use proc_macro2::TokenStream;
use quote::quote;

use crate::qol::PipeOp;

pub(crate) mod abi;
pub(crate) mod emit;
pub(crate) mod parsed;

pub(crate) fn apply(_attr: TokenStream, item: TokenStream) -> Result<TokenStream, syn::Error> {
    let mut contract = match crate::analysis::contract::decorated_elem(item.clone()) {
        crate::analysis::contract::DecoratedElem::Impl => parsed::ContractImpl::from_tokens(item),
    };

    let fns = contract.collect_fns()?;

    let match_arms = fns
        .iter()
        .map(|f| crate::contract::emit::emit_entry_call(&**f))
        .to(|x| quote! { #(#x),* });

    let mut attendum = quote! {

        extern crate alloc;


        // SAFETY: This application is single threaded, so using AssumeSingleThreaded is allowed.
        #[global_allocator]
        #[cfg(all(target_arch = "wasm32", target_os="unknown"))]
        static mut WASM_ALLOCATOR: syslib::allocator::AssumeSingleThreaded<
            syslib::allocator::BumpingAllocator> =
            unsafe { syslib::allocator::default_bumping_allocator() };

        #[no_mangle]
        pub extern "C" fn constructor() {
            #[cfg(all(target_arch = "wasm32", target_os="unknown"))]
            {
                syslib::init(unsafe { core::ptr::addr_of_mut!(WASM_ALLOCATOR) });
            }
        }

        #[no_mangle]
        #[allow(unused_must_use)]
        pub extern "C" fn runtime() -> &'static syslib::sys::SliceRef<usize> {
            use syslib::abi::Encodable;

            #[cfg(all(target_arch = "wasm32", target_os="unknown"))]
            {
                syslib::init(unsafe { core::ptr::addr_of_mut!(WASM_ALLOCATOR) });
            }

            let mut calldata = unsafe {
                syslib::abi::overlay::overlaid_calldata::OverlaidCalldata::new(
                    syslib::system::calldata::size() as usize,
                    |data| { syslib::system::calldata::read_into(data); }
                ).expect("Calldata couldn't be initialized.")
            };


            let selector = syslib::system::calldata::selector();
            let mut instance = Contract::default();

            let result = match selector {
                #match_arms,
                x => panic!("Unknown selector 0x{:08x?}.", x),
            };

            let result = match result {
                core::result::Result::Ok(result) => syslib::sys::SliceRef::<usize>::from_ref(result),
                core::result::Result::Err(msg) => panic!("{}", msg),
            };

            let result = alloc::boxed::Box::new(result);
            alloc::boxed::Box::leak(result)
        }
    };

    if std::env::var("ZK_FN_SELECTORS").is_ok() {
        // Print selectors
        println!("Function selectors:");
        for f in &fns {
            let s = f.selector().to_string();

            let le = u32::from_str_radix(&s.as_str()[2..], 16).unwrap();
            let be = le.to_be();


            println!(" - {:02x?} -> {}", s, f.name());
            println!(
                "   BE: 0x{:08x?}, LE: 0x{:08x?}",
                be,
                le
            );
            println!();
        }
    }

    drop(fns);

    attendum.extend(contract.emit_impl());
    Ok(attendum)
}
