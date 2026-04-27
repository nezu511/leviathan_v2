const { buildPoseidon } = require("circomlibjs");
const fs = require("fs");

async function run() {
    const poseidon = await buildPoseidon();
    const F = poseidon.F;

    // ユーザーしか知らない秘密の値とヌルファイア（本来は超巨大な乱数）
    const secret = 12345;
    const nullifier = 67890;

    // ZK回路と全く同じ Poseidon ハッシュを計算
    const commitment = F.toObject(poseidon([secret]));
    const nullifierHash = F.toObject(poseidon([secret, nullifier]));

    const input = {
        secret: secret.toString(),
        nullifier: nullifier.toString(),
        commitment: commitment.toString(),
        nullifierHash: nullifierHash.toString(),
        voteChoice: "1"
    };

    fs.writeFileSync("input.json", JSON.stringify(input, null, 2));
    console.log("✅ input.json を作成しました！");
    console.log("Commitment:", input.commitment);
    console.log("NullifierHash:", input.nullifierHash);
}
run();
