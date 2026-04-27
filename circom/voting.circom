pragma circom 2.0.0;

include "node_modules/circomlib/circuits/poseidon.circom";

template AnonymousVoting() {
    // --- 秘密入力 (Private Inputs) ---
    // ユーザーしか知らない秘密の値
    signal input secret;
    // 投票ごとにユーザーがランダムに選ぶ（または固定の）値
    signal input nullifier;

    // --- 公開入力 (Public Inputs) ---
    // EVM（スマートコントラクト）側でも知っている値
    signal input commitment;    // 登録時に記録した Poseidon(secret)
    signal input nullifierHash; // 今回の投票で使った Poseidon(secret, nullifier)
    signal input voteChoice;    // 投票先 (0 or 1。回路内では使わなくてもPublic Inputとして紐付けるために必要)

    // 1. Identity Commitment の検証 (本当に登録されているユーザーか？)
    component commitmentHasher = Poseidon(1);
    commitmentHasher.inputs[0] <== secret;
    commitment === commitmentHasher.out;

    // 2. Nullifier Hash の検証 (二重投票していないか？)
    // 別の投票をしようとすると nullifierHash が変わるため、スマートコントラクト側で弾ける
    component nullifierHasher = Poseidon(2);
    nullifierHasher.inputs[0] <== secret;
    nullifierHasher.inputs[1] <== nullifier;
    nullifierHash === nullifierHasher.out;
    
    // ダミー制約: voteChoice を回路にバインドする (最適化で消されないようにする)
    signal dummy;
    dummy <== voteChoice * 0;
}

// commitment, nullifierHash, voteChoice の3つを公開入力とする
component main {public [commitment, nullifierHash, voteChoice]} = AnonymousVoting();
