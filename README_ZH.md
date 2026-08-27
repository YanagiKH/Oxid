<div align="center">
  <img width="304" height="282" alt="Oxid 標誌" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />

  # Oxid

  **一個適合快速腳本、應用程式、套件與跨語言開發的精簡獨立語言。**

  [![儲存庫 CI](https://github.com/YanagiKH/Oxid/actions/workflows/ci.yml/badge.svg)](https://github.com/YanagiKH/Oxid/actions/workflows/ci.yml)
  [![版本](https://img.shields.io/github/v/release/YanagiKH/Oxid?include_prereleases)](https://github.com/YanagiKH/Oxid/releases)
  [![授權](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](LICENSE)

  [English](README.md) · [繁體中文](README_ZH.md) · [日本語](README_JP.md)
</div>

Oxid 0.9 是可直接使用的語言工具鏈，具備精簡與傳統語法、原始碼直譯、確定性的版本化成品、記錄與 JSON、可持續使用的網路轉接器、鎖定的路徑／Git 依賴、專案工具、原生 C/C++ 函式，以及 Python／Java／Go 程序橋接。一般使用者只需安裝一個執行檔，**不需要 Rust**。

## 專案狀態

Oxid 現在可用於腳本、自動化、教學、原型、本機 HTTP 服務、Discord 互動邏輯以及混合語言程序整合。發行執行檔內含解析器、執行環境、OXBC 編譯器／讀取器、套件解析器、效能測試框架、C/C++ 橋接、格式化工具、測試執行器、健康檢查及專案產生指令。

字節碼發射器已由 Oxid 編寫，啟動流程會檢查 stage-0／stage-1／stage-2 成品是否確定性相等。詞法分析器、解析器、診斷及模組提供者仍使用 Rust stage-0 啟動層，因此目前不宣稱已完成獨立自宿主。只有從原始碼建置該啟動層時才需要 Rust；使用發行執行檔撰寫、執行、檢查、編譯、管理套件或橋接 Oxid 程式時不需要 Rust。

## 為什麼選擇 Oxid

| 日常工作 | Rust 式繁瑣寫法 | Oxid 0.9 |
|---|---|---|
| 可變數值 | `let mut total = 0;` | `var total = 0;` |
| 輸出 | `println!("{value}");` | `say value;` |
| 短函式 | 函式區塊與明確回傳 | `fun double(n) => n * 2;` |
| 條件 | 強制使用 Rust 運算式語法 | `when ready { ... } otherwise { ... }` |
| 迭代 | 迭代器 trait 或手動迴圈 | `for item in values { ... }` |
| 管線 | 巢狀呼叫或轉接器 | `value |> clean |> encode;` |
| 非同步宣告 | 執行環境與 trait 設定 | `work fun fetch() => await request();` |
| 執行腳本 | 專案編譯流程 | `oxid run app.ox` |
| 單一成品 | 設定套件目標 | `oxid compile app.ox -o app.oxb` |
| 鎖定依賴 | 選擇並接入套件用戶端 | `oxid add codec <pinned-git-url>` |
| 結構化資料 | 加入序列化套件 | `{name: "Oxid"}`／`json_parse(text)` |
| 外部語言橋接 | 手動編寫主機端膠合程式 | `oxid bridge all bridges` |

Oxid 透過小型語言核心、預處理快取、單次解析模組，以及讓同一程式可直接執行或編成單一 `.oxb` 成品來提升開發效率。效能會隨工作負載改變；請使用 `oxid bench` 或應用程式專屬測量，不應假設相對 Rust 存在通用固定倍率。

## 架構

![Oxid 架構，顯示原始碼、前端、執行環境、套件、標準函式庫與橋接](docs/assets/architecture.svg)

- 詞法分析器與解析器同時理解傳統關鍵字及 Oxid 簡寫。
- 執行環境支援數字、字串、布林、null、陣列、確定性記錄、JSON、任務、可持續使用的 TCP 控制代碼、模組、檔案、程序、C/C++ 原生呼叫及具有限制的 HTTP 解析。
- 編譯器以確定順序解析匯入，並輸出含序列化 AST 版本 1、原始碼範圍、大小限制與負載校驗碼的 OXBC 1.0。
- 標準函式庫以 `.ox` 模組編寫，提供集合、文字、工作流程、Web 路由、Discord 分派及語言橋接說明。
- 自動產生的橋接 SDK 可讓外部主機一致地啟動 Oxid，而不必嵌入編譯器內部結構。
- `oxid.lock` 會記錄遞迴路徑依賴與固定 commit 的 Git 依賴，並附上套件樹校驗碼。

## 快速開始

![Oxid 終端機快速開始](docs/assets/quickstart.svg)

```bash
oxid new hello
cd hello
oxid run src/main.ox
oxid build
oxid test
```

產生的專案包含清單、空白且有效的 `oxid.lock`、原始碼入口、最小 prelude、範例、測試及建置腳本。`oxid build` 會驗證專案並產生 `.oxid/bin/hello.oxb`。

## 語言語法

### 傳統寫法

```oxid
fn double(value) {
    return value * 2;
}

fn main() {
    let values = range(1, 7);
    print map(values, double);
}
```

### Oxid 精簡寫法

```oxid
fun double(value) => value * 2;
fun label(value) => "value=" + str(value);

work fun greet(name) => "Hello, " + name;

fun main() {
    const values = range(1, 7);
    for value in values {
        when value % 2 == 0 { continue; }
        say value |> double |> label;
    }

    var job = greet("Oxid");
    say await job;
    say yes all (none == null);
}
```

這些簡寫是相容別名，而不是另一套不相容文法：`fun/fn`、`var/let`、`say/print`、`give/return`、`when/if`、`otherwise/else`、`loop/while`、`import/use`、`yes/true`、`no/false`、`none/null`、`all/and` 及 `any/or`。Oxid 也實作 `for … in`、`break`、`continue`、`%`、`|>`、`=>`、`async`、`await`、陣列、確定性記錄、屬性與字串鍵存取、賦值、註解與單行巨集。

## 安裝

### Linux 與 macOS 發行版安裝程式

安裝程式會偵測平台、下載最新版、驗證 SHA-256，並預設將 `oxid` 安裝至 `${HOME}/.local/bin`。

```bash
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/YanagiKH/Oxid/main/install.sh | sh
export PATH="$HOME/.local/bin:$PATH"
oxid --version
```

可設定 `OXID_INSTALL_DIR` 改變目錄，或設定 `OXID_VERSION=<release-tag>` 固定版本。已發布的 Unix 成品涵蓋 Linux x86_64、macOS x86_64 及 macOS arm64。

### Windows PowerShell 安裝程式

```powershell
Set-ExecutionPolicy -Scope Process Bypass
irm https://raw.githubusercontent.com/YanagiKH/Oxid/main/install.ps1 | iex
& "$env:LOCALAPPDATA\Oxid\bin\oxid.exe" --version
```

PowerShell 安裝程式會驗證壓縮檔校驗碼並支援 Windows x86_64。可使用 `OXID_INSTALL_DIR` 及 `OXID_VERSION` 覆寫預設值。

### 可攜式發行壓縮檔

1. 開啟 [GitHub Releases](https://github.com/YanagiKH/Oxid/releases)。
2. 下載對應作業系統的壓縮檔。
3. 使用旁邊的 `.sha256` 檔案驗證。
4. 將 `oxid` 或 `oxid.exe` 解壓至 `PATH` 內的目錄。

可攜式發行執行檔不需要任何語言執行環境。

### Cargo 或原始碼安裝

建置 stage-0 實作需要穩定版 Rust 以及 C/C++ 編譯器。

```bash
cargo install --git https://github.com/YanagiKH/Oxid --locked
# 或
git clone https://github.com/YanagiKH/Oxid.git
cd Oxid
make verify
sudo make install
```

### Docker

```bash
docker build -t oxid .
docker run --rm -v "$PWD:/workspace" oxid run /workspace/examples/hello.ox
```

容器會建置最佳化執行環境，並以非 root 使用者執行。

## 編譯與打包

```bash
oxid check src/main.ox
oxid compile src/main.ox -o app.oxb
oxid inspect app.oxb
oxid run app.oxb
oxid lock
oxid build --locked
oxid clean
```

`.oxb` 是 OXBC 1.0 成品，包含序列化 AST 版本 1、模組數、跨模組原始碼範圍及確定性負載校驗碼。執行環境會在執行前拒絕不相容、格式錯誤、過大、截斷或損壞的成品。`oxid ast` 會以 `.oxa` 副檔名寫出相同表示法。`oxid build` 會解析清單、驗證 `oxid.lock`，並在 `.oxid/` 下寫入應用程式成品與建置報告。

## 跨語言橋接

![Oxid 對 Python、Java、Go、C 與 C++ 的雙向橋接](docs/assets/interop.svg)

### 從 Oxid 呼叫外部程式

```oxid
fun main() {
    say python("-c", ["print('hello from Python')"]);
    say go("tools/report.go", ["--format", "json"]);
    say process_output("java", ["-jar", "service.jar"]);
    say c_hash("native");
    say cpp_hash("bridge");
}
```

`process` 回傳結束碼；`process_output` 回傳標準輸出，並將失敗結束狀態轉成 Oxid 錯誤。`python`、`java` 與 `go` 提供精簡轉接。原生 `c_len`、`c_hash`、`cpp_len` 及 `cpp_hash` 會在每次 CI 建置中證明 ABI 邊界確實完成連結。

### 從其他語言呼叫 Oxid

```bash
oxid bridge python bridges/python
oxid bridge java bridges/java
oxid bridge go bridges/go
oxid bridge c bridges/c
oxid bridge cpp bridges/cpp
# 一次產生全部 SDK：
oxid bridge all bridges
```

產生的檔案使用各生態系統的標準程序 API，並公開小型 `run` 入口。這能保持協定穩定，同時讓主機端膠合層可替換。使用 C/C++ shell 轉接器時，請只使用受信任的檔名與命令參數。

## Web 模組

![Oxid Web 路由與 Discord 互動模組](docs/assets/web-discord.svg)

```oxid
import "stdlib/web.ox";

fun health(body) => web_json(200, "{\"status\":\"ok\"}");
fun echo(body) => web_text(200, body);

fun main() {
    const routes = [
        web_route_entry("GET", "/health", health),
        web_route_entry("POST", "/echo", echo)
    ];
    const response = web_dispatch(routes, "GET", "/health", "");
    web_serve_once("127.0.0.1", 8080, response);
}
```

`stdlib/web.ox` 提供路由項目、本機分派及文字／JSON 回應。原生 `net_listen`、`net_accept`、`net_try_accept`、`net_read`、`net_write`、`http_read_request`、`http_write_response` 與 `net_close` 提供可重複使用、具有資料與逾時限制的非阻塞 socket；`web_serve_once` 仍適合簡單的單次請求程式。TLS、身分驗證、流量限制及正式環境排程仍由轉接器負責。

## Discord 模組

```oxid
import "stdlib/bots/discord.ox";

fun ping(payload) => discord_reply("Pong: " + payload);

fun main() {
    const commands = [discord_command("ping", "Reply with pong", ping)];
    say discord_dispatch(commands, "ping", "interaction-data");
}
```

此模組可建立 Discord 互動回應、註冊指令、分派負載，並透過 `discord_run_adapter` 啟動 gateway 轉接器。使用 `oxid discord new my-bot` 產生可讀取 token 的專案骨架。HTTPS 及 WebSocket gateway 傳輸會隔離在可替換的轉接器，而不是固定寫死在語言核心中。

## 指令參考

| 指令 | 用途 |
|---|---|
| `oxid run <file>` | 執行 `.ox` 原始碼或已驗證的 `.oxb`／`.oxa` 成品 |
| `oxid check <file>` | 只進行詞法、預處理與解析，不執行 |
| `oxid compile <file> [-o output]` | 產生確定性的 OXBC 成品 |
| `oxid ast <file> [-o output]` | 輸出版本化的序列化 AST |
| `oxid inspect <artifact>` | 顯示成品版本、數量及校驗碼 |
| `oxid repl` | 啟動互動式直譯器 |
| `oxid new/init <name>` | 建立一般專案骨架 |
| `oxid web new <name>` | 建立 Web 專案骨架 |
| `oxid discord new <name>` | 建立 Discord bot 專案骨架 |
| `oxid bridge <target> [output]` | 產生 Python／Java／Go／C／C++ 主機 SDK |
| `oxid build [依賴旗標]` | 解析、鎖定並建立 `.oxid/bin/*.oxb` |
| `oxid test` | 執行語言煙霧測試與核心範例 |
| `oxid fmt [path]` | 格式化單一原始碼或整個專案 |
| `oxid watch <file>` | 專案檔案變更後重新執行 |
| `oxid script <name> [args]` | 執行 `oxid.toml` 腳本 |
| `oxid add <name> <target>` | 新增依賴項目 |
| `oxid remove/list/lock/fetch/update/install` | 管理路徑與固定 commit 的 Git 依賴 |
| `oxid bench [選項]` | 測量冷啟動、解析、打包及執行操作 |
| `oxid doctor` | 檢查專案結構 |
| `oxid doc` | 產生內建 API 文件 |
| `oxid clean` | 移除 `.oxid` 快取／建置目錄 |
| `oxid bootstrap/self-host [--check]` | 驗證或寫入確定性的編譯器階段成品 |

## 儲存庫結構

```text
Oxid/
├── src/                  # stage-0 解析器、執行環境、CLI、套件器
├── compiler/             # Oxid 編譯器入口與前端提供者清單
├── stdlib/               # 以 Oxid 編寫的標準模組
│   ├── interop/          # C、C++、Python、Java、Go 橋接輔助
│   └── bots/discord.ox   # Discord 指令與回應模組
├── examples/             # 可執行語言、Web、bot 與橋接範例
├── tests/                # Oxid 煙霧測試程式
├── tools/                # 以 Oxid 編寫的專案／工具鏈腳本
├── native/               # 已連結的 C 與 C++ ABI 實作
├── scripts/              # 儲存庫及發行驗證
├── docs/assets/          # README 圖表
└── .github/workflows/    # 完整 CI 與校驗碼發行建置
```

## 驗證與發行

每次 push 與 pull request 都會執行：

- Rust 格式檢查，以及將警告視為錯誤的 Clippy；
- 語法、OXBC 往返、原始碼範圍、記錄／JSON、網路限制、套件鎖定、啟動比對、橋接及原生 C/C++ 連結單元測試；
- 對每個 `.ox` 檔案進行語法檢查；
- 執行所有測試、範例、工具、應用程式與套件 demo；
- 在 Linux x86_64、Windows x86_64、macOS x86_64 及 macOS arm64 進行最佳化建置；
- README 內容對齊、SVG XML、TOML、JSON、workflow、原始碼安裝及 Docker 檢查；
- 執行專案 `test`、鎖定 `build`、啟動比對、效能報告結構及 `doctor` 指令。

版本標籤只會在可重用 CI 工作流程成功後，才打包獨立壓縮檔、產生 SHA-256 檔案並發布至 GitHub Releases。

## 獨立性與發展路線

Oxid 0.9 已提供版本化成品、Oxid 編寫的發射器、確定性啟動比對及明確的提供者清單。發行版使用者不需安裝 Rust 即可使用 `oxid`、`.ox`、`.oxb` 與 `.oxa`。詞法分析器、解析器、診斷及模組提供者仍屬 stage-0；這些元件會逐項驗證後替換，只有在獨立的跨平台等效性得到證實後，自宿主路徑才會成為預設發行路徑。

## 安全性

程序橋接會執行 Oxid 應用程式指定的程式。請勿把不受信任的執行檔路徑或 shell 片段交給自動產生的 C/C++ 轉接器。Git 依賴必須使用 HTTPS 與完整 commit，並應審查鎖定檔的校驗碼變更。網路與 JSON 解析器具有限制，但不提供 TLS 或應用程式身分驗證。請依 [SECURITY.md](SECURITY.md) 私下回報漏洞。

## 貢獻與授權

請閱讀 [CONTRIBUTING.md](CONTRIBUTING.md)、執行 `make verify`，並確保公開儲存庫文件是提供所有使用者閱讀。Oxid 採用 [MIT](LICENSE) 或 [Apache-2.0](LICENSE-APACHE) 授權。
