@REM libsqlite3-sys 的 bundled-sqlcipher 特性需要外部提供的 OpenSSL。
@REM 把这两个变量指到一个含 include/openssl 与 lib/libcrypto.lib 的安装目录，
@REM 所有 cargo 命令（build / test / clippy）都必须带着它们跑。
@REM 样例：
set "OPENSSL_LIB_DIR=D:\path\to\openssl\install\lib"
set "OPENSSL_INCLUDE_DIR=D:\path\to\openssl\install\include"
