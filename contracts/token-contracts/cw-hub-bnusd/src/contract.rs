#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdResult, Uint128, QueryRequest, WasmQuery, Empty,
};
// use cw2::set_contract_version;
use crate::constants::{REPLY_MSG_SUCCESS, X_CROSS_TRANSFER, X_CROSS_TRANSFER_REVERT};
use crate::error::ContractError;
use crate::state::{HUB_ADDRESS, HUB_NET, NID, OWNER, X_CALL, X_CALL_BTP_ADDRESS};
use bytes::Bytes;
use cw_common::hub_token_msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use cw_common::x_call_msg::{XCallMsg, XCallQuery};

use cw20_base::contract::{execute_burn, execute_mint};
use cw20_base::state::{MinterData, TokenInfo, TOKEN_INFO};
use cw_common::network_address::NetworkAddress;

use common::rlp::{DecoderError, Rlp};

use cw_common::data_types::types::{CrossTransfer, CrossTransferRevert};

/*
// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw-hub-bnusd";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
*/

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // create initial accounts
    // store token info using cw20-base format
    let data = TokenInfo {
        name: "HubToken".to_string(),
        symbol: "HUBT".to_string(),
        decimals: 18,
        total_supply: Uint128::zero(),
        // set self as minter, so we can properly execute mint and burn
        mint: Some(MinterData {
            minter: _env.contract.address,
            cap: None,
        }),
    };
    let save_token = TOKEN_INFO.save(deps.storage, &data);
    if save_token.is_err() {
        return Err(ContractError::Std(save_token.err().unwrap()));
    }
    deps.api
        .addr_validate(&msg.x_call)
        .expect("ContractError::InvalidToAddress");
    let xcall = X_CALL.save(deps.storage, &msg.x_call);
    if xcall.is_err() {
        return Err(ContractError::Std(xcall.err().unwrap()));
    }
    let _x_call = &msg.x_call;
    let query_message = XCallQuery::GetNetworkAddress {
    };

    let query: QueryRequest<Empty> = QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: _x_call.to_string(),
        msg: to_binary(&query_message).map_err(ContractError::Std)?,
    });

    let x_call_btp_address: String = deps.querier.query(&query).map_err(ContractError::Std)?;

    // let x_call_btp_address = "btp://0x38.bsc/0x034AaDE86BF402F023Aa17E5725fABC4ab9E9798";
    if x_call_btp_address.is_empty() {
        return Err(ContractError::AddressNotFound);
    }
    let (nid, _) = NetworkAddress::parse_network_address(&x_call_btp_address)?;
    let (hub_net, hub_address) = NetworkAddress::parse_protocol_address(&msg.hub_address)?;

    X_CALL_BTP_ADDRESS.save(deps.storage, &x_call_btp_address.to_string())?;
    NID.save(deps.storage, &nid.to_string())?;
    HUB_ADDRESS.save(deps.storage, &hub_address.to_string())?;
    HUB_NET.save(deps.storage, &hub_net.to_string())?;
    OWNER.save(deps.storage, &_info.sender)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    use ExecuteMsg::*;
    match msg {
        Setup {
            x_call,
            hub_address,
        } => execute::setup(deps, _env, info, x_call, hub_address),
        HandleCallMessage { from, data } => {
            execute::handle_call_message(deps, _env, info, from, data)
        }
        CrossTransfer { to, amount, data } => {
            execute::cross_transfer(deps, _env, info, to, amount, data.into())
        }
        XCrossTransfer {
            from,
            cross_transfer_data,
        } => execute::x_cross_transfer(deps, _env, info, from, cross_transfer_data),
        XCrossTransferRevert {
            from,
            cross_transfer_revert_data,
        } => execute::x_cross_transfer_revert(deps, _env, info, from, cross_transfer_revert_data),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
    unimplemented!()
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        REPLY_MSG_SUCCESS => reply::reply_msg_success(deps, env, msg),
        _ => Err(ContractError::InvalidReply),
    }
}

mod reply {
    use super::*;

    pub fn reply_msg_success(
        _deps: DepsMut,
        _env: Env,
        _msg: Reply,
    ) -> Result<Response, ContractError> {
        Ok(Response::default())
    }
}

mod execute {
    use cosmwasm_std::{to_binary, CosmosMsg, Empty, QueryRequest, SubMsg, WasmQuery};

    use super::*;

    pub fn setup(
        deps: DepsMut,
        _env: Env,
        _info: MessageInfo,
        x_call: String,
        hub_address: String,
    ) -> Result<Response, ContractError> {
        deps.api
            .addr_validate(&x_call)
            .expect("ContractError::InvalidToAddress");
        X_CALL.save(deps.storage, &x_call)?;
        //Network address call remaining
        let query_message = XCallQuery::GetNetworkAddress {
        };

        let query: QueryRequest<Empty> = QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: x_call,
            msg: to_binary(&query_message).map_err(ContractError::Std)?,
        });

        let x_call_btp_address: String = deps.querier.query(&query).map_err(ContractError::Std)?;
        if x_call_btp_address.is_empty() {
            return Err(ContractError::AddressNotFound);
        }
        let (nid, _) = NetworkAddress::parse_network_address(&x_call_btp_address)?;
        let (hub_net, hub_address) = NetworkAddress::parse_protocol_address(&hub_address)?;

        X_CALL_BTP_ADDRESS.save(deps.storage, &x_call_btp_address)?;
        NID.save(deps.storage, &nid.to_string())?;
        HUB_ADDRESS.save(deps.storage, &hub_address.to_string())?;
        HUB_NET.save(deps.storage, &hub_net.to_string())?;
        OWNER.save(deps.storage, &_info.sender)?;
        Ok(Response::default())
    }

    pub fn handle_call_message(
        deps: DepsMut,
        _env: Env,
        info: MessageInfo,
        from: String,
        data: Vec<u8>,
    ) -> Result<Response, ContractError> {
        deps.api
            .addr_validate(&from)
            .expect("ContractError::InvalidToAddress");
        let rlp: Rlp = Rlp::new(&data);
        let data: Result<Vec<String>, DecoderError> = rlp.as_list();
        match data {
            Ok(decoded_data) => {
                let method = &decoded_data[0];

                match method.as_str() {
                    X_CROSS_TRANSFER => {
                        let cross_transfer_data: CrossTransfer =
                            rlpdecode_struct::decode_cross_transfer(&decoded_data);
                        x_cross_transfer(deps, _env, info, from, cross_transfer_data)?;
                    }
                    X_CROSS_TRANSFER_REVERT => {
                        let cross_transfer_revert_data: CrossTransferRevert =
                            rlpdecode_struct::decode_cross_transfer_revert(&decoded_data);
                        x_cross_transfer_revert(
                            deps,
                            _env,
                            info,
                            from,
                            cross_transfer_revert_data,
                        )?;
                    }
                    _ => {
                        return Err(ContractError::InvalidMethod);
                    }
                }
            }
            Err(_error) => return Err(ContractError::InvalidData),
        }

        Ok(Response::default())
    }

    pub fn cross_transfer(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        to: String,
        amount: u128,
        data: Bytes,
    ) -> Result<Response, ContractError> {
        use super::*;
        deps.api
            .addr_validate(&to)
            .expect("ContractError::InvalidToAddress");
        let funds = info.funds.clone();
        let nid = NID.load(deps.storage)?;
        let hub_net: String = HUB_NET.load(deps.storage)?;
        let hub_address: String = HUB_ADDRESS.load(deps.storage)?;

        let from = NetworkAddress::get_network_address("btp",&nid, info.sender.as_ref());

        let _call_data = CrossTransfer {
            from: from.clone(),
            to,
            value: amount,
            data: data.to_vec(),
        };

        let _rollback_data = CrossTransferRevert {
            from,
            value: amount,
        };

        let _hub_btp_address = NetworkAddress::get_network_address("btp",&hub_net, &hub_address);

        let call_message = XCallMsg::SendCallMessage {
            to: _hub_btp_address,
            data: _call_data.encode_cross_transfer_message(),
            rollback: Some(_rollback_data.encode_cross_transfer_revert_message()),
        };

        let wasm_execute_message: CosmosMsg = CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
            contract_addr: X_CALL.load(deps.storage).unwrap(),
            msg: to_binary(&call_message)?,
            funds,
        });

        let sub_message = SubMsg::reply_always(wasm_execute_message, REPLY_MSG_SUCCESS);
        let _result = execute_burn(deps, env, info, amount.into());
        match _result {
            Ok(resp) => {
                print!("this is {:?}", resp)
            }
            Err(_error) => {
                return Err(ContractError::MintError);
            }
        }
        Ok(Response::new()
            .add_submessage(sub_message)
            .add_attribute("method", "cross_transfer"))
    }

    pub fn x_cross_transfer(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        from: String,
        cross_transfer_data: CrossTransfer,
    ) -> Result<Response, ContractError> {
        deps.api
            .addr_validate(&from)
            .expect("ContractError::InvalidToAddress");
        let nid = NID.load(deps.storage)?;
        let hub_net: String = HUB_NET.load(deps.storage)?;
        let hub_address: String = HUB_ADDRESS.load(deps.storage)?;

        let btp_address = NetworkAddress::get_network_address("btp",&hub_net, &hub_address);

        if from != btp_address {
            return Err(ContractError::Unauthorized {});
        }

        let (net, account) = NetworkAddress::parse_protocol_address(&cross_transfer_data.to)?;
        if net != nid {
            return Err(ContractError::WrongNetwork);
        }

        let _to = deps
            .api
            .addr_validate(account)
            .expect("ContractError::InvalidToAddress");

        let res = execute_mint(
            deps,
            env,
            info,
            account.to_string(),
            cross_transfer_data.value.into(),
        )
        .expect("Fail to mint");

        Ok(res)
    }

    pub fn x_cross_transfer_revert(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        from: String,
        cross_transfer_revert_data: CrossTransferRevert,
    ) -> Result<Response, ContractError> {
        deps.api
            .addr_validate(&from)
            .expect("ContractError::InvalidToAddress");
        let nid = NID.load(deps.storage)?;
        let x_call_btp_address = X_CALL_BTP_ADDRESS.load(deps.storage)?;

        if from != x_call_btp_address {
            return Err(ContractError::OnlyCallService);
        }

        let (net, account) =
            NetworkAddress::parse_protocol_address(&cross_transfer_revert_data.from)?;
        if net != nid {
            return Err(ContractError::InvalidBTPAddress);
        }

        let _to = deps
            .api
            .addr_validate(account)
            .expect("ContractError::InvalidToAddress");

        let res = execute_mint(
            deps,
            env,
            info,
            account.to_string(),
            cross_transfer_revert_data.value.into(),
        )
        .expect("Fail to mint");

        Ok(res)
    }
}

mod rlpdecode_struct {
    use super::*;
    pub fn decode_cross_transfer(ls: &[String]) -> CrossTransfer {
        CrossTransfer {
            from: ls[1].clone(),
            to: ls[2].clone(),
            value: ls[3].parse::<u128>().unwrap_or_default(),
            data: ls[4].clone().into(),
        }
    }

    pub fn decode_cross_transfer_revert(ls: &[String]) -> CrossTransferRevert {
        CrossTransferRevert {
            from: ls[1].clone(),
            value: ls[2].parse::<u128>().unwrap_or_default(),
        }
    }
}
#[cfg(test)]
mod rlp_test {
    use common::rlp::{DecoderError, Rlp, RlpStream};

    #[test]
    fn test() {
        let bytes: Vec<u8> = [
            248, 133, 142, 120, 67, 114, 111, 115, 115, 84, 114, 97, 110, 115, 102, 101, 114, 184,
            57, 98, 116, 112, 58, 47, 47, 48, 120, 51, 56, 46, 98, 115, 99, 47, 48, 120, 48, 51,
            52, 65, 97, 68, 69, 56, 54, 66, 70, 52, 48, 50, 70, 48, 50, 51, 65, 97, 49, 55, 69, 53,
            55, 50, 53, 102, 65, 66, 67, 52, 97, 98, 57, 69, 57, 55, 57, 56, 184, 56, 98, 116, 112,
            58, 47, 47, 48, 120, 49, 46, 105, 99, 111, 110, 47, 48, 120, 48, 51, 65, 97, 68, 69,
            56, 54, 66, 70, 52, 48, 50, 70, 48, 50, 51, 65, 97, 49, 55, 69, 53, 55, 50, 53, 102,
            65, 66, 67, 52, 97, 98, 57, 69, 57, 55, 57, 56, 100, 132, 100, 97, 116, 97,
        ]
        .into();
        let rlp: Rlp = Rlp::new(&bytes);
        let ddata: Result<Vec<String>, DecoderError> = rlp.as_list();

        let mut _decoded_data: Vec<String> = Vec::new();

        print!("this is {:?} {:?} {:?}", bytes, ddata, rlp)
    }
    #[test]

    fn encodetest() {
        let method = "xCrossTransfer";
        let val: u32 = 100;

        let mut calldata = RlpStream::new_list(4);
        calldata.append(&method.to_string());
        calldata.append(&"btp://0x38.bsc/0x034AaDE86BF402F023Aa17E5725fABC4ab9E9798");
        calldata.append(&"btp://0x1.icon/0x03AaDE86BF402F023Aa17E5725fABC4ab9E9798");
        calldata.append(&val);
        calldata.append(&"data");

        let encoded = calldata.as_raw().to_vec();
        print!("this is {:?}", encoded)
    }
}

#[cfg(test)]
mod tests_instantiate {
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};

    use super::*;

    #[test]
    fn setup() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("SENDER", &[]);
        let msg = InstantiateMsg{
            x_call: "todo!()".to_owned(),
            hub_address: "todo!()".to_owned(),
        };

        let _res: Response = instantiate(deps.as_mut(), env, info.clone(), msg).unwrap();
        // let mut deps = mock_dependencies();
        // let env = mock_env();

        // instantiate(
        //     deps.as_mut(),
        //     env.clone(),
        //     mock_info("sender", &[]),
        //     InstantiateMsg { x_call: "()".to_owned(), hub_address: "()".to_owned() },
        // )
        // .unwrap();

        // let resp = query(deps.as_ref(), env, QueryMsg::Greet {}).unwrap();
        // let resp: GreetResp = from_binary(&resp).unwrap();
        // assert_eq!(
        //     resp,
        //     GreetResp {
        //         message: "Hello World".to_owned()
        //     }
        // );
    }
}
#[cfg(test)]
mod contract_test {
    use cosmwasm_std::Addr;
    use cw_multi_test::{ContractWrapper, App, Executor};

    use super::*;


#[test]
fn init() {
    let mut app = App::default();

        let code: ContractWrapper<ExecuteMsg, InstantiateMsg, QueryMsg, ContractError, ContractError, cosmwasm_std::StdError> = ContractWrapper::new(execute, instantiate, query);
        let code_id = app.store_code(Box::new(code));

        let _addr = app.
        instantiate_contract(code_id, 
                            Addr::unchecked("owner"), 
                            &InstantiateMsg{
                                x_call: "archway13zjt2swjk0un2fpp3259szed7dsfmv3etdfkumrstlrdcq3szx9szucncp".to_owned(),
                                hub_address: "btp://0x1.icon/0x03AaDE86BF402F023Aa17E5725fABC4ab9E9798".to_owned(),
                            },
                            &[], 
                            "Contract", 
                            None)
                            .unwrap();
        
}

}
