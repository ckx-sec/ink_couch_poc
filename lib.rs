#![cfg_attr(not(feature = "std"), no_std)]

use ink_env::Environment;
use ink_lang as ink;
use serde_json::Value;

#[ink::chain_extension]
pub trait CouchBridge {
    type ErrorCode = CouchErr;

    #[ink(extension = 2, returns_result = false)]
    fn db_find_raw(subject: String) -> String;

    #[ink(extension = 3, returns_result = false)]
    fn db_create_raw(subject: String) -> String;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum CouchErr {
    FailGetRandomSource,
}

impl ink_env::chain_extension::FromStatusCode for CouchErr {
    fn from_status_code(status_code: u32) -> Result<(), Self> {
        match status_code {
            0 => Ok(()),
            1 => Err(Self::FailGetRandomSource),
            _ => panic!("encountered unknown status code"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum CustomEnvironment {}

impl Environment for CustomEnvironment {
    const MAX_EVENT_TOPICS: usize = <ink_env::DefaultEnvironment as Environment>::MAX_EVENT_TOPICS;

    type AccountId = <ink_env::DefaultEnvironment as Environment>::AccountId;
    type Balance = <ink_env::DefaultEnvironment as Environment>::Balance;
    type Hash = <ink_env::DefaultEnvironment as Environment>::Hash;
    type BlockNumber = <ink_env::DefaultEnvironment as Environment>::BlockNumber;
    type Timestamp = <ink_env::DefaultEnvironment as Environment>::Timestamp;

    type ChainExtension = CouchBridge;
}

#[ink::contract(env = crate::CustomEnvironment)]
mod my_contract {
    use super::CouchErr;
    use couch_rs::types::find::FindQuery;
    use serde_json::{json, Value};

    #[ink(storage)]
    pub struct MyContract {}

    impl MyContract {
        #[ink(constructor)]
        pub fn new() -> Self {
            Self {}
        }

        #[ink(message)]
        pub fn find_all(&self) -> String {
            let find_all = FindQuery::find_all();
            let find_all_str = find_all.as_value().to_string();
            let result_raw = self.env().extension().db_find_raw(find_all_str).unwrap();
            //let result: Value = serde_json::from_slice(&result_raw).unwrap();
            let result_value = serde_json::from_str::<Value>(&result_raw).unwrap();
            serde_json::to_string_pretty(&result_value).unwrap()
        }

        #[ink(message)]
        pub fn create(&self) -> String {
            let test_doc = json!({
                "ink":"substrate",
                "this":"that",
                "asdf":"zxcv"
            });
            self.env()
                .extension()
                .db_create_raw(test_doc.to_string())
                .unwrap()
        }
    }

    /// Unit tests in Rust are normally defined within such a `#[cfg(test)]`
    #[cfg(test)]
    mod tests {
        use crate::my_contract::*;
        /// Imports all the definitions from the outer scope so we can use them here.
        use couch_rs::types::find::FindQuery;
        use ink_lang as ink;
        use serde_json::{json, Value};
        // use futures::executor::block_on;

        #[ink::test]
        fn find_all_works() {
            struct FindExtension;
            impl ink_env::test::ChainExtension for FindExtension {
                fn func_id(&self) -> u32 {
                    2
                }

                fn call(&mut self, _input: &[u8], output: &mut Vec<u8>) -> u32 {
                    const TEST_DB: &str = "test_db";
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();
                    runtime.block_on(async {
                        use scale::Decode;

                        let client = couch_rs::Client::new_local_test().unwrap();
                        let db = client.db(TEST_DB).await.unwrap();

                        let query_value = {
                            let mut input = _input;
                            // 这里ink把input编码了两次，原因不明，只好解码两次
                            let input_decode: Vec<u8> = Vec::decode(&mut input).unwrap();
                            let query_str = String::decode(&mut input_decode.as_slice()).unwrap();
                            let mut value: Value = serde_json::from_str(&query_str).unwrap();

                            let obj = value.as_object_mut().unwrap();
                            if !obj.contains_key("sort") {
                                obj.insert("sort".to_owned(), json!([]));
                            };

                            value
                        };

                        let query = FindQuery::new_from_value(query_value);
                        let docs = db.find::<Value>(&query).await.unwrap();

                        let result = {
                            let result_raw = docs.get_data();
                            serde_json::to_string(result_raw).unwrap()
                        };
                        scale::Encode::encode_to(&result, output);
                    });

                    0
                }
            }
            ink_env::test::register_chain_extension(FindExtension);
            let contract = MyContract::new();
            println!("{}", contract.find_all());
        }

        #[ink::test]
        fn create_works() {
            struct CreateExtension;
            impl ink_env::test::ChainExtension for CreateExtension {
                fn func_id(&self) -> u32 {
                    3
                }

                fn call(&mut self, _input: &[u8], output: &mut Vec<u8>) -> u32 {
                    const TEST_DB: &str = "test_db";
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();

                    runtime.block_on(async {
                        let client = couch_rs::Client::new_local_test().unwrap();
                        let db = client.db(TEST_DB).await.unwrap();

                        use scale::Decode;

                        let mut doc: Value = {
                            let mut input = _input;
                            // 这里ink把input编码了两次，原因不明，只好解码两次
                            let input_decode: Vec<u8> = Vec::decode(&mut input).unwrap();
                            let query_raw = String::decode(&mut input_decode.as_slice()).unwrap();
                            serde_json::from_str(&query_raw).unwrap()
                        };

                        let result = {
                            let result_raw = db.create(&mut doc).await.unwrap();
                            serde_json::to_string(&result_raw).unwrap()
                        };
                        scale::Encode::encode_to(&result, output);
                    });

                    0
                }
            }
            ink_env::test::register_chain_extension(CreateExtension);
            let contract = MyContract::new();
            println!("{}", contract.create());
        }
    }
}
