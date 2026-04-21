// SPDX-License-Identifier: MIT
pragma solidity ^0.5.16;

contract MyNumberVerifier {
    // 0.5.x では address(10) と記述します (10 = 0x0a)
    address constant RSA_PRECOMPILE = address(10);

    function verifySignature(
        bytes memory signature,
        bytes memory modulus,
        bytes32 messageHash,
        bytes memory exponent
    ) public view returns (bool) {
        bytes memory payload = abi.encodePacked(signature, modulus, messageHash, exponent);

        // 0.5.x では staticcall の戻り値の受け取り方が少し異なります
        (bool success, bytes memory returnData) = RSA_PRECOMPILE.staticcall(payload);

        require(success, "Precompile call failed");

        // 32バイトの returnData を bool にデコード
        return abi.decode(returnData, (bool));
    }
}
