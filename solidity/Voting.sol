// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract Voting {
    address public constant GROTH16_PRECOMPILE = address(0x0b);
    address public vkContract;
    
    mapping(bytes32 => bool) public spentNullifiers;
    mapping(uint256 => uint256) public votes;

    // コンストラクタで VK_Data.bin をデプロイした先のアドレスを受け取る
    constructor(address _vkContract) {
        vkContract = _vkContract;
    }

    function castVote(
        bytes calldata proof,         // 256 bytes (A, B, C)
        bytes32 nullifierHash,        // Public Input 1
        bytes32 root,                 // Public Input 2
        uint256 voteChoice            // Public Input 3
    ) external {
        require(!spentNullifiers[nullifierHash], "Already voted");

        uint256 vkSize = vkContract.code.length;
        require(vkSize > 0, "VK contract is empty");

        // my_groth16 が期待するレイアウト:
        // [vkSize(32)] + [VK Data(vkSize)] + [Proof(256)] + [Inputs(96)]
        bytes memory payload = new bytes(32 + vkSize + 256 + 96);

        assembly {
            // 1. VK_length (32バイト)
            mstore(add(payload, 32), vkSize)

            // 2. VK本体のコピー (EXTCODECOPY)
            extcodecopy(sload(vkContract.slot), add(payload, 64), 0, vkSize)

            // 3. Proofのコピー
            let proofOffset := add(payload, add(64, vkSize))
            calldatacopy(proofOffset, proof.offset, 256)

	    // 4. Public Inputs のコピー (Circomの定義順: [commitment, nullifierHash, voteChoice])
            let inputsOffset := add(proofOffset, 256)
            mstore(inputsOffset, root)                   // 1番目: commitment (root)
            mstore(add(inputsOffset, 32), nullifierHash) // 2番目: nullifierHash
            mstore(add(inputsOffset, 64), voteChoice)    // 3番目: voteChoice
        }

        // プレコンパイル 0x0b 呼び出し
        (bool success, bytes memory returnData) = GROTH16_PRECOMPILE.staticcall(payload);
        require(success, "Precompile reverted");
        
        uint256 isValid = abi.decode(returnData, (uint256));
        require(isValid == 1, "Invalid ZK Proof");

        // 状態更新
        spentNullifiers[nullifierHash] = true;
        votes[voteChoice] += 1;
    }
}
