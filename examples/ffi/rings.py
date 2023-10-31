import platform
import re

import cffi
from eth_account.messages import encode_defunct
from web3 import Web3

w3 = Web3()
acc = w3.eth.account.create()

ffi = cffi.FFI()
c_header = open("./target/include/rings.h", "r").read()
c_header = re.sub(r"#define .*", "", c_header)
ffi.cdef(c_header)

current_os = platform.system()
if current_os == "Windows":
    extension = "dll"
elif current_os == "Darwin":
    extension = "dylib"
else:
    extension = "so"


@ffi.callback("void (*)(const char *, char *)")
def signer(msg, output):
    c_input = ffi.string(msg)
    decoded = encode_defunct(c_input)
    sig = acc.sign_message(decoded)
    print("signature", sig.signature)
    print("signature len", len(sig.signature))
    ffi.memmove(output, sig.signature, len(sig.signature))
    print("output", ffi.string(output))
    return


@ffi.callback("void(*)(const char *, const char *)")
def custom_msg_callback(msg):
    print(msg)
    return


@ffi.callback("void(*)(const char *)")
def builtin_msg_callback(msg):
    print(msg)
    return


def create_client(rings_node, acc):
    callback = rings_node.new_callback(custom_msg_callback, builtin_msg_callback)
    client = rings_node.new_client_with_callback(
        "stun://stun.l.google.com".encode(),
        10,
        acc.address.encode(),
        "eip191".encode(),
        signer,
        ffi.addressof(callback),
    )
    return client


if __name__ == "__main__":
    rings_node = ffi.dlopen(f"./target/debug/librings_node.{extension}")
    rings_node.init_logging(rings_node.Debug)

    client = create_client(rings_node, acc)
    print(client)