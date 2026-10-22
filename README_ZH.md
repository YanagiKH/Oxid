<img align="right" width="96" alt="Oxid 標誌" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />

# Oxid

**以 Rust 為靈感，用來撰寫腳本與命令列工具的程式語言。**

寫好 `.ox` 檔案，就能用 `oxid` 執行。已發布的實驗版 0.9 將直譯器、
格式化工具、測試與專案管理功能整合在同一個執行檔中。

[安裝](docs/INSTALLATION.md) · [快速入門](docs/QUICKSTART.md) · [文件](docs/README.md)

[English](README.md) · [繁體中文](README_ZH.md) · [日本語](README_JP.md)

## 馬上試試

從 [Releases](https://github.com/YanagiKH/Oxid/releases) 下載適合你平台的壓縮檔，
解壓縮後將 `oxid` 或 `oxid.exe` 加入 `PATH`。使用發布版不需要安裝 Rust。
[安裝指南](docs/INSTALLATION.md)也提供安裝程式與從原始碼建置的步驟。

將以下程式存成 `double.ox`：

```oxid
fn main() {
    for n in [1, 2, 3] {
        print n * 2;
    }
}
```

執行：

```sh
oxid run double.ox
```

程式會逐行印出 `2`、`4`、`6`。

## 建立專案

```sh
oxid new hello
cd hello
oxid run src/main.ox
oxid test
oxid build
oxid run .oxid/bin/hello.oxb
```

起始程式會印出 `Hello from Oxid`。修改 `src/main.ox` 就能改變程式內容。
`oxid build` 會產生 `.oxid/bin/hello.oxb`，可用相容版本的 Oxid 執行。
資料處理與專案工具的用法，請見[快速入門](docs/QUICKSTART.md)。

## 開發版本的原生專案

開發中的原始碼也支援具有明確型別與所有權檢查的專案，並可產生原生執行檔。
這項實驗性工作流程**尚未包含在已發布的 0.9 執行檔中**。

原生建置目前需要 **Linux x86_64**、**LLVM/Clang/LLD 19.1.7**，
以及系統的 C 開發工具。原生程式目前設有執行步數與資源上限，詳情請見下方指南。

請從[原生專案指南](docs/TYPED_PROJECTS.md)開始，了解建立、檢查、測試與建置專案的方法，
以及本機依賴、工作區和原始碼套件的用法。
[位元組計數應用程式](apps/byte-count/README.md)示範如何將完整程式拆分成多個套件。

## 指南與範例

- [語言語法](docs/SYNTAX.md)與[內建函式](docs/API.md)
- [模組](docs/MODULES.md)與[腳本依賴](docs/PACKAGES.md)
- [指令參考](docs/COMMANDS.md)：格式化、測試與專案工具
- [呼叫其他語言](docs/INTEROP.md)
- [範例程式](examples/)

## 參與開發

要建置 Oxid 本身，請依照[原始碼安裝步驟](docs/INSTALLATION.md#build-a-checkout)操作。
提交修改與回報問題，請見[貢獻指南](CONTRIBUTING.md)與
[Issues](https://github.com/YanagiKH/Oxid/issues)。

採用 [MIT](LICENSE) 或 [Apache-2.0](LICENSE-APACHE) 授權。
