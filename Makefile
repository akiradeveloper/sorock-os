.PHONY: docker
docker:
	docker build -t sorock:dev -f sorock/Dockerfile .