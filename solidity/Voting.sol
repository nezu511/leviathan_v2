// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

// 🌟 追加：IdentityRegistryの関数を呼ぶためのインターフェース
interface IIdentityRegistry {
    function isValidRoot(bytes32 root) external view returns (bool);
}

contract Voting {
    address public vkContract;
    IIdentityRegistry public registry; // 🌟 追加：登録所のアドレスを保持
    
    mapping(bytes32 => bool) public spentNullifiers;
    mapping(uint256 => uint256) public votes;

    // 🌟 変更：コンストラクタで登録所のアドレスも受け取る
    constructor(address _vkContract, address _registryAddr) {
        vkContract = _vkContract;
        registry = IIdentityRegistry(_registryAddr);
    }

    function castVote(
        bytes memory proof,
        bytes32 nullifierHash,
        bytes32 root,
        uint256 voteChoice
    ) external {
        // 🌟 【最重要】公式名簿のRootと一致しなければ弾く！
        require(registry.isValidRoot(root), "Invalid Root: Voter not in the official registry");

        // 二重投票のチェック
        require(!spentNullifiers[nullifierHash], "Double voting: This nullifier is already spent");

        // --- 以下、既存の my_groth16 呼び出しロジックそのまま ---
        bytes memory vk_code = vkContract.code;
        require(vk_code.length > 0, "VK contract has no code");

        bytes memory payload = abi.encodePacked(
            uint256(vk_code.length),
            vk_code,
            proof,
            root,
            nullifierHash,
            voteChoice
        );

        address GROTH16_PRECOMPILE = address(0x0b);
        (bool success, bytes memory returnData) = GROTH16_PRECOMPILE.staticcall(payload);
        require(success, "Groth16 verification failed");

        uint256 isValid = abi.decode(returnData, (uint256));
        require(isValid == 1, "Mathematical proof is invalid");

        spentNullifiers[nullifierHash] = true;
        votes[voteChoice] += 1;
    }
}
