use toy_merkle_radix_account_trie::{Account, Address, State, build_genesis_state};

fn main() {
    let alice: Address = [0x11u8; 20];
    let bob: Address = [0x22u8; 20];

    let alice_account = Account::new_eoa(0, 1_000);
    let bob_account = Account::new_eoa(0, 50);
    let state = build_genesis_state(vec![
        (alice, alice_account.clone()),
        (bob, bob_account.clone()),
    ]);

    let root = state.root_hash();
    let loaded_alice = state
        .get_account(alice)
        .expect("alice should exist in genesis state");
    let proof = state
        .prove_account(alice)
        .expect("alice proof should exist");
    let valid = State::verify_account_proof(root, alice, &loaded_alice, &proof);

    assert_eq!(loaded_alice, alice_account);
    assert_eq!(state.get_account(bob), Some(bob_account));
    assert!(valid);

    let fake_alice = Account::new_eoa(0, 999_999);
    assert!(!State::verify_account_proof(
        root,
        alice,
        &fake_alice,
        &proof
    ));

    println!("state root: 0x{}", hex::encode(root));
    println!("alice proof nodes: {}", proof.len());
    println!("alice proof valid: {valid}");
}
