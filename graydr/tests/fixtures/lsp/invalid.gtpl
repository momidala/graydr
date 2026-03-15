template "lsp_test_invalid" {
  resource "broken" {
    module = "nonexistent"
    inputs = {}
  }
}
