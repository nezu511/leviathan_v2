// SPDX-License-Identifier: MIT
pragma solidity ^0.5.16;

contract MyNumberVerifier {
    address constant RSA_PRECOMPILE = address(10);
    function verifySignature(bytes memory payload) public view returns (bool) {
        (bool success, bytes memory returnData) = RSA_PRECOMPILE.staticcall(payload);
        require(success, "Precompile call failed");
        return abi.decode(returnData, (bool));
    }
}
