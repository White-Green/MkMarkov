import requests
import time
import os
import json

host = os.environ.get("MISSKEY_INSTANCE_HOST", "voskey.icalo.net")
key = os.environ["MISSKEY_API_KEY"]
username = "white_green"

search_user_endpoint = "https://%s/api/users/search-by-username-and-host" % host
notes_endpoint = "https://%s/api/users/notes" % host

response = requests.post(search_user_endpoint, json={"i": key, "username": username, "host": host, "detail": False})
user_id = response.json()[0]["id"]

all_notes_list = []
until = None
print("collecting user data")
while True:
    print(until)
    if until:
        response = requests.post(notes_endpoint, json={"i": key, "userId": user_id, "limit": 100, "local": True, "untilId": until})
    else:
        response = requests.post(notes_endpoint, json={"i": key, "userId": user_id, "limit": 100, "local": True})
    notes = response.json()
    if len(notes) == 0:
        break
    all_notes_list += notes
    until = all_notes_list[-1]["id"]
    time.sleep(1)

with open("all_notes.json", "w") as f:
    f.write(json.dumps(all_notes_list))
