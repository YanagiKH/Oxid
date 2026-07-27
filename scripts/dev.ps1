param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $Args
)

cargo run -- @Args
