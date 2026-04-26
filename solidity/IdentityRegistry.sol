// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract IdentityRegistry {
    mapping(bytes32 => bool) public isRegistered;
    address constant RSA_PRECOMPILE = address(0x0a);

    function register(
        bytes memory modulus,
        bytes memory exponent,
        bytes memory signature,
        bytes memory message,
        bytes32 commitment
    ) public {
        // 1. メッセージをSHA256でハッシュ化 
        // (Rust側の Pkcs1v15Sign::verify が32バイトのハッシュ値を要求するため)
        bytes32 hashedMessage = sha256(message);

        // 2. Rust側 (my_rsa) の get_padded_data のオフセットと完全に一致させる
        // 順序: Signature(256) -> Modulus(256) -> HashedMessage(32) -> Exponent(残り)
        bytes memory payload = abi.encodePacked(
            signature,
            modulus,
            hashedMessage,
            exponent
        );

        // 3. プレコンパイル呼び出し
        (bool success, ) = RSA_PRECOMPILE.staticcall(payload);
        
        require(success, "RSA verification failed");
        isRegistered[commitment] = true;
    }
}
