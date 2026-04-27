// circom/generate_input.js
const { buildPoseidon } = require("circomlibjs");
const fs = require("fs");

async function main() {
    const poseidon = await buildPoseidon();
    const F = poseidon.F;

    // あなたの秘密情報
    const secret = "12345";
    const nullifier = "67890";
    const voteChoice = "1";

    const toStr = (buf) => F.toString(buf);

    // Commitment と NullifierHash
    const commitmentBuf = poseidon([secret, nullifier]);
    const nullifierHashBuf = poseidon([nullifier, 1]);

    const levels = 20;
    let pathElements = [];
    let pathIndices = [];

    // 🌟 修正: Solidity側のロジック（常に空ノードは0x0）に完全一致させる
    for (let i = 0; i < levels; i++) {
        pathElements.push("0"); // EVMは bytes32(0) を使っているため、ここも 0 に固定
        pathIndices.push(0);    // 1人目の登録者なので道順は常に左(0)
    }

    // JS内でRootを計算 (Solidityと全く同じ計算過程をたどる)
    let currentHash = F.toObject(commitmentBuf);
    for (let i = 0; i < levels; i++) {
        // Solidityの `currentNode = poseidon(currentNode, bytes32(0))` を再現
        const nextHashBuf = poseidon([currentHash, 0n]);
        currentHash = F.toObject(nextHashBuf);
    }
    const finalRootStr = currentHash.toString();
    console.log("Calculated Root in JS (Matching EVM!):", finalRootStr);

    // 4. input.json を出力
    const inputJson = {
        root: finalRootStr,
        nullifierHash: toStr(nullifierHashBuf),
        voteChoice: voteChoice,
        secret: secret,
        nullifier: nullifier,
        pathElements: pathElements,
        pathIndices: pathIndices
    };

    fs.writeFileSync("input.json", JSON.stringify(inputJson, null, 2));
    console.log("✅ input.json generated successfully!");
}

main().catch(console.error);
