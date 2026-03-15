template "completion_context" {
  metadata {
    name    = "completion_context"
    version = "1.0.0"
    description = "Fixture template for LSP completion tests"
    authors = ["test"]
    tags    = ["test"]
    lifecycle {
      maturity = "stable"
    }
  }

  parameters {
    provider = { type = "string" }
  }

  resource "test-resource" {
    module = "completion_context_module"
    inputs = {
      vpc_id = "vpc-123"
      size   = "m"
      name   = "test"
    }
  }

  outputs {}
}
