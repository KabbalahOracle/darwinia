// Copyright 2018-2019 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! The Substrate runtime. This can be compiled with ``#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]

/// Constant values used within the runtime.
pub mod constants;
/// Implementations of some helper traits passed into runtime modules as associated types.
pub mod impls;

pub use contracts::Gas;
pub use timestamp::Call as TimestampCall;

pub use balances::Call as BalancesCall;
pub use staking::StakerStatus;

use grandpa::{fg_primitives, AuthorityList as GrandpaAuthorityList};
use im_online::sr25519::AuthorityId as ImOnlineId;
use inherents::{CheckInherentsResult, InherentData};
use node_primitives::{AccountId, AccountIndex, Balance, BlockNumber, Hash, Index, Moment, Signature};
use rstd::prelude::*;
use sr_api::impl_runtime_apis;
use sr_primitives::{
	create_runtime_str, generic, impl_opaque_keys,
	traits::{self, BlakeTwo256, Block as BlockT, NumberFor, OpaqueKeys, SaturatedConversion, StaticLookup},
	transaction_validity::TransactionValidity,
	weights::Weight,
	ApplyResult, Perbill,
};
use substrate_primitives::{
	u32_trait::{_1, _4},
	OpaqueMetadata,
};
use support::{
	construct_runtime, parameter_types,
	traits::{Currency, OnUnbalanced, Randomness, SplitTwoWays},
};
use system::offchain::TransactionSubmitter;
use transaction_payment_rpc_runtime_api::RuntimeDispatchInfo;
#[cfg(any(feature = "std", test))]
use version::NativeVersion;
use version::RuntimeVersion;

use constants::{currency::*, time::*};
use impls::{Author, CurrencyToVoteHandler, LinearWeightToFee, TargetedFeeAdjustment};

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));
/// Runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("node"),
	impl_name: create_runtime_str!("darwinia-node"),
	authoring_version: 3,
	spec_version: 84,
	impl_version: 84,
	apis: RUNTIME_API_VERSIONS,
};

/// Native version.
#[cfg(any(feature = "std", test))]
pub fn native_version() -> NativeVersion {
	NativeVersion {
		runtime_version: VERSION,
		can_author_with: Default::default(),
	}
}

type NegativeImbalance = <Balances as Currency<AccountId>>::NegativeImbalance;
type DealWithFees = SplitTwoWays<
	Balance,
	NegativeImbalance,
	_4,
	MockTreasury, // 4 parts (80%) goes to the treasury.
	_1,
	Author, // 1 part (20%) goes to the block author.
>;

pub struct MockTreasury;
impl OnUnbalanced<NegativeImbalance> for MockTreasury {
	fn on_unbalanced(amount: NegativeImbalance) {
		Balances::resolve_creating(&Sudo::key(), amount);
	}
}

parameter_types! {
	pub const BlockHashCount: BlockNumber = 250;
	pub const MaximumBlockWeight: Weight = 1_000_000_000;
	pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
	pub const MaximumBlockLength: u32 = 5 * 1024 * 1024;
	pub const Version: RuntimeVersion = VERSION;
}
impl system::Trait for Runtime {
	type Origin = Origin;
	type Call = Call;
	type Index = Index;
	type BlockNumber = BlockNumber;
	type Hash = Hash;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = Indices;
	type Header = generic::Header<BlockNumber, BlakeTwo256>;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = Version;
}

impl utility::Trait for Runtime {
	type Event = Event;
	type Call = Call;
}

parameter_types! {
	pub const EpochDuration: u64 = EPOCH_DURATION_IN_SLOTS;
	pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
}
impl babe::Trait for Runtime {
	type EpochDuration = EpochDuration;
	type ExpectedBlockTime = ExpectedBlockTime;
	type EpochChangeTrigger = babe::ExternalTrigger;
}

impl indices::Trait for Runtime {
	type AccountIndex = AccountIndex;
	type IsDeadAccount = Balances;
	type ResolveHint = indices::SimpleResolveHint<Self::AccountId, Self::AccountIndex>;
	type Event = Event;
}

parameter_types! {
	// Develop
	//	pub const TransactionBaseFee: Balance = MICRO;
	//	pub const TransactionByteFee: Balance = MICRO;
	// setting this to zero will disable the weight fee.
	//	pub const WeightFeeCoefficient: Balance = MICRO;

	// Production
	pub const TransactionBaseFee: Balance = 1 * MICRO;
	pub const TransactionByteFee: Balance = 10 * MICRO;
	// setting this to zero will disable the weight fee.
	pub const WeightFeeCoefficient: Balance = 50 * NANO;

	// for a sane configuration, this should always be less than `AvailableBlockRatio`.
	pub const TargetBlockFullness: Perbill = Perbill::from_percent(25);
}
impl transaction_payment::Trait for Runtime {
	type Currency = Balances;
	type OnTransactionPayment = DealWithFees;
	type TransactionBaseFee = TransactionBaseFee;
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = LinearWeightToFee<WeightFeeCoefficient>;
	type FeeMultiplierUpdate = TargetedFeeAdjustment<TargetBlockFullness>;
}

parameter_types! {
	pub const MinimumPeriod: Moment = SLOT_DURATION / 2;
}
impl timestamp::Trait for Runtime {
	type Moment = Moment;
	type OnTimestampSet = Babe;
	type MinimumPeriod = MinimumPeriod;
}

impl_opaque_keys! {
	pub struct SessionKeys {
		pub grandpa: Grandpa,
		pub babe: Babe,
		pub im_online: ImOnline,
	}
}

parameter_types! {
	pub const UncleGenerations: BlockNumber = 5;
}
impl authorship::Trait for Runtime {
	type FindAuthor = session::FindAccountFromAuthorIndex<Self, Babe>;
	type UncleGenerations = UncleGenerations;
	type FilterUncle = ();
	type EventHandler = (Staking, ImOnline);
}

// NOTE: `SessionHandler` and `SessionKeys` are co-dependent: One key will be used for each handler.
// The number and order of items in `SessionHandler` *MUST* be the same number and order of keys in
// `SessionKeys`.
// TODO: Introduce some structure to tie these together to make it a bit less of a footgun. This
// should be easy, since OneSessionHandler trait provides the `Key` as an associated type. #2858

parameter_types! {
	pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17);
}
impl session::Trait for Runtime {
	type Event = Event;
	type ValidatorId = <Self as system::Trait>::AccountId;
	type ValidatorIdOf = staking::StashOf<Self>;
	type ShouldEndSession = Babe;
	type OnSessionEnding = Staking;
	type SessionHandler = <SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
	type Keys = SessionKeys;
	type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
	type SelectInitialValidators = Staking;
}

impl session::historical::Trait for Runtime {
	type FullIdentification = staking::Exposure<AccountId, Balance>;
	type FullIdentificationOf = staking::ExposureOf<Runtime>;
}

parameter_types! {
	// Develop
	//	pub const ContractTransferFee: Balance = MICRO;
	//	pub const ContractCreationFee: Balance = MICRO;
	//	pub const ContractTransactionBaseFee: Balance = MICRO;
	//	pub const ContractTransactionByteFee: Balance = MICRO;
	//	pub const ContractFee: Balance = MICRO;
	//	pub const TombstoneDeposit: Balance = MICRO;
	//	pub const RentByteFee: Balance = MICRO;
	//	pub const RentDepositOffset: Balance = MICRO;
	//	pub const SurchargeReward: Balance = MICRO;

	// Production
	pub const ContractTransferFee: Balance = 1 * MICRO;
	pub const ContractCreationFee: Balance = 1 * MICRO;
	pub const ContractTransactionBaseFee: Balance = 1 * MICRO;
	pub const ContractTransactionByteFee: Balance = 10 * MICRO;
	pub const ContractFee: Balance = 1 * MICRO;
	pub const TombstoneDeposit: Balance = 1 * COIN;
	pub const RentByteFee: Balance = 1 * COIN;
	pub const RentDepositOffset: Balance = 1000 * COIN;
	pub const SurchargeReward: Balance = 150 * COIN;
}
impl contracts::Trait for Runtime {
	type Currency = Balances;
	type Time = Timestamp;
	type Randomness = RandomnessCollectiveFlip;
	type Call = Call;
	type Event = Event;
	type DetermineContractAddress = contracts::SimpleAddressDeterminator<Runtime>;
	type ComputeDispatchFee = contracts::DefaultDispatchFeeComputor<Runtime>;
	type TrieIdGenerator = contracts::TrieIdFromParentCounter<Runtime>;
	type GasPayment = ();
	type RentPayment = ();
	type SignedClaimHandicap = contracts::DefaultSignedClaimHandicap;
	type TombstoneDeposit = TombstoneDeposit;
	type StorageSizeOffset = contracts::DefaultStorageSizeOffset;
	type RentByteFee = RentByteFee;
	type RentDepositOffset = RentDepositOffset;
	type SurchargeReward = SurchargeReward;
	type TransferFee = ContractTransferFee;
	type CreationFee = ContractCreationFee;
	type TransactionBaseFee = ContractTransactionBaseFee;
	type TransactionByteFee = ContractTransactionByteFee;
	type ContractFee = ContractFee;
	type CallBaseFee = contracts::DefaultCallBaseFee;
	type InstantiateBaseFee = contracts::DefaultInstantiateBaseFee;
	type MaxDepth = contracts::DefaultMaxDepth;
	type MaxValueSize = contracts::DefaultMaxValueSize;
	type BlockGasLimit = contracts::DefaultBlockGasLimit;
}

impl sudo::Trait for Runtime {
	type Event = Event;
	type Proposal = Call;
}

type SubmitTransaction = TransactionSubmitter<ImOnlineId, Runtime, UncheckedExtrinsic>;
parameter_types! {
	pub const SessionDuration: BlockNumber = SESSION_DURATION;
}
impl im_online::Trait for Runtime {
	type AuthorityId = ImOnlineId;
	type Event = Event;
	type Call = Call;
	type SubmitTransaction = SubmitTransaction;
	type SessionDuration = SessionDuration;
	type ReportUnresponsiveness = Offences;
}

impl offences::Trait for Runtime {
	type Event = Event;
	type IdentificationTuple = session::historical::IdentificationTuple<Self>;
	type OnOffenceHandler = Staking;
}

impl grandpa::Trait for Runtime {
	type Event = Event;
}

parameter_types! {
	pub const WindowSize: BlockNumber = 101;
	pub const ReportLatency: BlockNumber = 1000;
}
impl finality_tracker::Trait for Runtime {
	type OnFinalizationStalled = Grandpa;
	type WindowSize = WindowSize;
	type ReportLatency = ReportLatency;
}

impl system::offchain::CreateTransaction<Runtime, UncheckedExtrinsic> for Runtime {
	type Public = <Signature as traits::Verify>::Signer;
	type Signature = Signature;

	fn create_transaction<F: system::offchain::Signer<Self::Public, Self::Signature>>(
		call: Call,
		public: Self::Public,
		account: AccountId,
		index: Index,
	) -> Option<(Call, <UncheckedExtrinsic as traits::Extrinsic>::SignaturePayload)> {
		let period = 1 << 8;
		let current_block = System::block_number().saturated_into::<u64>();
		let tip = 0;
		let extra: SignedExtra = (
			system::CheckVersion::<Runtime>::new(),
			system::CheckGenesis::<Runtime>::new(),
			system::CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
			system::CheckNonce::<Runtime>::from(index),
			system::CheckWeight::<Runtime>::new(),
			transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
			Default::default(),
		);
		let raw_payload = SignedPayload::new(call, extra).ok()?;
		let signature = F::sign(public, &raw_payload)?;
		let address = Indices::unlookup(account);
		let (call, extra, _) = raw_payload.deconstruct();
		Some((call, (address, signature, extra)))
	}
}

parameter_types! {
	pub const ExistentialDeposit: Balance = COIN;
	pub const TransferFee: Balance = MICRO;
	pub const CreationFee: Balance = MICRO;
}
impl balances::Trait for Runtime {
	type Balance = Balance;
	type OnFreeBalanceZero = ((Staking, Contracts), Session);
	type OnNewAccount = Indices;
	type TransferPayment = ();
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type TransferFee = TransferFee;
	type CreationFee = CreationFee;
}
impl kton::Trait for Runtime {
	type Event = Event;
}

parameter_types! {
	pub const SessionsPerEra: sr_staking_primitives::SessionIndex = SESSION_PER_ERA;
	// about 14 days = 14 * 24 * 60 * 60 * 1000
	pub const BondingDuration: Moment = 1_209_600_000;
	pub const BondingDurationInEra: staking::EraIndex = 4032;
	// decimal 9
	pub const HardCap: Balance = 1_000_000_000 * COIN;
	// Date in Los Angeles*: 12/25/2019, 10:58:29 PM
	// Date in Berlin* :12/26/2019, 1:58:29 PM
	// Date in Beijing*: 12/26/2019, 12:58:29 PM
	// Date in New York* :12/26/2019, 12:58:29 AM
	pub const GenesisTime: Moment = 1_577_339_909_000;
}
impl staking::Trait for Runtime {
	type Time = Timestamp;
	type CurrencyToVote = CurrencyToVoteHandler;
	type Event = Event;
	type SessionsPerEra = SessionsPerEra;
	type BondingDuration = BondingDuration;
	type BondingDurationInEra = BondingDurationInEra;
	type SessionInterface = Self;
	type Ring = Balances;
	type RingRewardRemainder = ();
	type RingSlash = ();
	type RingReward = ();
	type Kton = Kton;
	type KtonSlash = ();
	type KtonReward = ();

	type Cap = HardCap;
	type GenesisTime = GenesisTime;
}

parameter_types! {
	pub const EthMainet: u64 = 0;
	pub const EthRopsten: u64 = 1;
}

impl eth_relay::Trait for Runtime {
	type Event = Event;
	type EthNetwork = EthRopsten;
}

impl eth_backing::Trait for Runtime {
	type Event = Event;
	type EthRelay = EthRelay;
	type Ring = Balances;
	type Kton = Kton;
	type OnDepositRedeem = Staking;
	type DetermineAccountId = eth_backing::AccountIdDeterminator<Runtime>;
	type RingReward = ();
	type KtonReward = ();
}

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = node_primitives::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		// Basic stuff; balances is uncallable initially.
		RandomnessCollectiveFlip: randomness_collective_flip::{Module, Call, Storage},
		System: system::{Module, Call, Storage, Event, Config},

		// Must be before session.
		Babe: babe::{Module, Call, Storage, Config, Inherent(Timestamp)},

		Balances: balances::{default, Error},
		Indices: indices,
		Kton: kton,
		Timestamp: timestamp::{Module, Call, Storage, Inherent},
		TransactionPayment: transaction_payment::{Module, Storage},

		// Consensus support.
		Authorship: authorship::{Module, Call, Storage, Inherent},
		Grandpa: grandpa::{Module, Call, Storage, Event, Config},
		ImOnline: im_online::{default, ValidateUnsigned},
		FinalityTracker: finality_tracker::{Module, Call, Inherent},
		Offences: offences::{Module, Call, Storage, Event},
		Session: session::{Module, Call, Storage, Event, Config<T>},
		Staking: staking::{default, OfflineWorker},

		Contracts: contracts,
		Sudo: sudo,
		Utility: utility::{Module, Call, Event},
		
		EthRelay: eth_relay::{Module, Call, Storage, Event<T>, Config<T>},
		EthBacking: eth_backing,
	}
);

/// The address format for describing accounts.
pub type Address = <Indices as StaticLookup>::Source;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	system::CheckVersion<Runtime>,
	system::CheckGenesis<Runtime>,
	system::CheckEra<Runtime>,
	system::CheckNonce<Runtime>,
	system::CheckWeight<Runtime>,
	transaction_payment::ChargeTransactionPayment<Runtime>,
	contracts::CheckBlockGasLimit<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<Call, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, Call, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = executive::Executive<Runtime, Block, system::ChainContext<Runtime>, Runtime, AllModules>;

impl_runtime_apis! {
	impl sr_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}
	}

	impl sr_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			Runtime::metadata().into()
		}
	}

	impl block_builder_api::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(block: Block, data: InherentData) -> CheckInherentsResult {
			data.check_extrinsics(&block)
		}

		fn random_seed() -> <Block as BlockT>::Hash {
			RandomnessCollectiveFlip::random_seed()
		}
	}

	impl tx_pool_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(tx: <Block as BlockT>::Extrinsic) -> TransactionValidity {
			Executive::validate_transaction(tx)
		}
	}

	impl offchain_primitives::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(number: NumberFor<Block>) {
			Executive::offchain_worker(number)
		}
	}

	impl fg_primitives::GrandpaApi<Block> for Runtime {
		fn grandpa_authorities() -> GrandpaAuthorityList {
			Grandpa::grandpa_authorities()
		}
	}

	impl babe_primitives::BabeApi<Block> for Runtime {
		fn configuration() -> babe_primitives::BabeConfiguration {
			// The choice of `c` parameter (where `1 - c` represents the
			// probability of a slot being empty), is done in accordance to the
			// slot duration and expected target block time, for safely
			// resisting network delays of maximum two seconds.
			// <https://research.web3.foundation/en/latest/polkadot/BABE/Babe/#6-practical-results>
			babe_primitives::BabeConfiguration {
				slot_duration: Babe::slot_duration(),
				epoch_length: EpochDuration::get(),
				c: PRIMARY_PROBABILITY,
				genesis_authorities: Babe::authorities(),
				randomness: Babe::randomness(),
				secondary_slots: true,
			}
		}
	}

	impl system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
		fn account_nonce(account: AccountId) -> Index {
			System::account_nonce(account)
		}
	}

	impl contracts_rpc_runtime_api::ContractsApi<Block, AccountId, Balance> for Runtime {
		fn call(
			origin: AccountId,
			dest: AccountId,
			value: Balance,
			gas_limit: u64,
			input_data: Vec<u8>,
		) -> contracts_rpc_runtime_api::ContractExecResult {
			use contracts_rpc_runtime_api::ContractExecResult;

			let exec_result = Contracts::bare_call(
				origin,
				dest.into(),
				value,
				gas_limit,
				input_data,
			);
			match exec_result {
				Ok(v) => ContractExecResult::Success {
					status: v.status,
					data: v.data,
				},
				Err(_) => ContractExecResult::Error,
			}
		}

		fn get_storage(
			address: AccountId,
			key: [u8; 32],
		) -> contracts_rpc_runtime_api::GetStorageResult {
			Contracts::get_storage(address, key).map_err(|rpc_err| {
				use contracts::GetStorageError;
				use contracts_rpc_runtime_api::{GetStorageError as RpcGetStorageError};
				/// Map the contract error into the RPC layer error.
				match rpc_err {
					GetStorageError::ContractDoesntExist => RpcGetStorageError::ContractDoesntExist,
					GetStorageError::IsTombstone => RpcGetStorageError::IsTombstone,
				}
			})
		}
	}

	impl transaction_payment_rpc_runtime_api::TransactionPaymentApi<
		Block,
		Balance,
		UncheckedExtrinsic,
	> for Runtime {
		fn query_info(uxt: UncheckedExtrinsic, len: u32) -> RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
	}

	impl substrate_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			SessionKeys::generate(seed)
		}
	}
}
