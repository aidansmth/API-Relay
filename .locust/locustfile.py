# Used to stress test the API relay
# Using locust from https://locust.io/

from locust import HttpUser, task, between

class QuickstartUser(HttpUser):
    @task
    def get(self):
        self.client.get("/get")