template "test_queue" {
  metadata {
    description = "Compile test for queue module."
    version     = "0.0.1"
  }

  parameters {
    queue {
      provider            = {}
      name                = {}
      region              = {}
      resource_group_name = {}
    }
  }

  resource "main_queue" {
    module = "queue"
    inputs {
      name                = "$queue.name"
      region              = "$queue.region"
      resource_group_name = "$queue.resource_group_name"
    }
  }

  outputs {
    queue_url  = "${main_queue.queue_url}"
    queue_name = "${main_queue.queue_name}"
  }
}
