openssl req -x509 -nodes -days 365 -newkey rsa:2048 -keyout nginx/ssl/server.key -out nginx/ssl/server.crt -subj "/CN=stager.test" -addext "subjectAltName=DNS:stager.test,IP:192.168.10.230"
