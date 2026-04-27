// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract IdentityRegistry {
    mapping(bytes32 => bool) public isRegistered;
    address constant RSA_PRECOMPILE = address(0x0a);

    // 🌟 追加: Poseidon Merkle Tree の状態変数
    uint256 public constant TREE_DEPTH = 20; // 最大 2^20 人まで登録可能
    bytes32[20] public filledSubtrees;
    uint256 public nextIndex = 0;
    bytes32 public currentRoot;

    function register(
        bytes memory modulus,
        bytes memory exponent,
        bytes memory signature,
        bytes memory message,
        bytes32 commitment
    ) public {
        // 二重登録の防止
        require(!isRegistered[commitment], "Already registered");

        // 1. RSA検証
        bytes32 hashedMessage = sha256(message);
        bytes memory payload = abi.encodePacked(
            signature,
            modulus,
            hashedMessage,
            exponent
        );

        (bool success, ) = RSA_PRECOMPILE.staticcall(payload);
        require(success, "RSA verification failed");
        
        isRegistered[commitment] = true;

        // 🌟 2. 検証成功後、Poseidonツリーへ挿入
        _insertToTree(commitment);
    }

    // 🌟 追加: ツリー更新ロジック
    function _insertToTree(bytes32 leaf) internal {
        require(nextIndex < 2**TREE_DEPTH, "Tree is full");

        bytes32 currentNode = leaf;
        uint256 index = nextIndex;

        for (uint256 i = 0; i < TREE_DEPTH; i++) {
            if (index % 2 == 0) {
                filledSubtrees[i] = currentNode;
                // 左側に入った場合、右側はまだ空（0x0）としてハッシュ計算
                currentNode = poseidon(currentNode, bytes32(0));
            } else {
                // 右側に入った場合、保存しておいた左側(filledSubtrees)と結合
                currentNode = poseidon(filledSubtrees[i], currentNode);
            }
            index /= 2;
        }

        currentRoot = currentNode; // これが ZK 投票の検証に使う「公式Root」になる
        nextIndex += 1;
    }

    function isValidRoot(bytes32 root) external view returns (bool) {
        return root == currentRoot;
    }

    // 2つの bytes32 を受け取り、Poseidonハッシュ化して 1つの bytes32 を返す
    function poseidon(bytes32 left, bytes32 right) internal view returns (bytes32) {
        bytes memory payload = abi.encodePacked(uint8(1), left, right);
        bytes32 result;

        assembly {
            let success := staticcall(gas(), 0x0c, add(payload, 32), 65, 0x00, 32)

            if iszero(success) { revert(0, 0) }

            result := mload(0x00)
        }
        return result;
    }
}
