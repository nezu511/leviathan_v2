pragma circom 2.0.0;

// ※ご自身の環境に合わせて poseidon.circom のパスを調整してください
include "node_modules/circomlib/circuits/poseidon.circom";

// 左右のノードを正しく並べてハッシュ化するためのヘルパー
template DualMux() {
    signal input in[2];
    signal input s; // 0 (左) or 1 (右)
    signal output out[2];

    out[0] <== (in[1] - in[0])*s + in[0];
    out[1] <== (in[0] - in[1])*s + in[1];
}

// 匿名投票のメイン回路
template VotingCircuit(levels) {
    // ==========================================
    // 🌟 Public Inputs (誰にでも見える公開情報)
    // ==========================================
    signal input root;          // 市役所の公式Root (Solidityがチェックするもの)
    signal input nullifierHash; // 使用済みスタンプ (二重投票防止用)
    signal input voteChoice;    // 投票先

    // ==========================================
    // 🔒 Private Inputs (スマホの中に隠しておく秘密情報)
    // ==========================================
    signal input secret;
    signal input nullifier;
    signal input pathElements[levels]; // ツリーの兄弟ノードたちのハッシュ
    signal input pathIndices[levels];  // 自分が左(0)か右(1)かの道順

    // 1. Commitment (葉) の計算
    component commitmentHasher = Poseidon(2);
    commitmentHasher.inputs[0] <== secret;
    commitmentHasher.inputs[1] <== nullifier;
    signal leaf <== commitmentHasher.out;

    // 2. Merkle Tree の Root を下から上へ計算する
    component hashers[levels];
    component mux[levels];

    signal levelHashes[levels + 1];
    levelHashes[0] <== leaf;

    for (var i = 0; i < levels; i++) {
        mux[i] = DualMux();
        mux[i].in[0] <== levelHashes[i];
        mux[i].in[1] <== pathElements[i];
        mux[i].s <== pathIndices[i]; // 0なら自分が左、1なら自分が右

        hashers[i] = Poseidon(2);
        hashers[i].inputs[0] <== mux[i].out[0];
        hashers[i].inputs[1] <== mux[i].out[1];

        levelHashes[i + 1] <== hashers[i].out;
    }

    // 3. 【超重要】計算して導き出したRootが、公開されている公式Rootと一致するか検証！
    root === levelHashes[levels];

    // 4. Nullifier Hash の計算と検証
    component nullifierHasher = Poseidon(2);
    nullifierHasher.inputs[0] <== nullifier;
    nullifierHasher.inputs[1] <== 1; // ドメイン分離（ハッシュの衝突を防ぐため）
    nullifierHash === nullifierHasher.out;

    // ※ voteChoice を Public Input として回路に認識させるためのダミー制約
    signal dummy <== voteChoice * 0;
}

// ツリーの深さ20 (IdentityRegistry.sol と同じ) でコンポーネントを作成
// Public Input として扱う変数を指定
component main {public [root, nullifierHash, voteChoice]} = VotingCircuit(20);
