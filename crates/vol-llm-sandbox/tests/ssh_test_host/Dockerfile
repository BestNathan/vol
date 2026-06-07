FROM alpine:3.20
RUN apk add --no-cache openssh bash ripgrep
RUN ssh-keygen -A
RUN adduser -D agent && echo "agent:password" | chpasswd
RUN mkdir -p /home/agent/sandbox && chown agent:agent /home/agent/sandbox
RUN mkdir /run/sshd
COPY id_ed25519.pub /home/agent/.ssh/authorized_keys
RUN chmod 600 /home/agent/.ssh/authorized_keys && chown -R agent:agent /home/agent/.ssh
EXPOSE 22
CMD ["/usr/sbin/sshd", "-D", "-e"]
