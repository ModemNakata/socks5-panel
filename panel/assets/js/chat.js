(() => {
    'use strict';

    const messagesContainer = document.getElementById('chat-messages');
    const messageInput = document.getElementById('message-input');
    const sendBtn = document.getElementById('send-btn');
    const connectionStatus = document.getElementById('connection-status');

    let lastMessageId = null;
    let isSending = false;
    const syncInterval = 3000;
    let syncTimer = null;

    const formatTime = (date) => {
        return date.toLocaleTimeString('en-US', { 
            hour: 'numeric', 
            minute: '2-digit',
            hour12: true 
        });
    };

    const scrollToBottom = () => {
        requestAnimationFrame(() => {
            messagesContainer.scrollTop = messagesContainer.scrollHeight;
        });
    };

    const createMessageElement = (text, type = 'user', createdAt = null) => {
        const messageDiv = document.createElement('div');
        messageDiv.className = `message ${type}`;

        const contentDiv = document.createElement('div');
        contentDiv.className = 'message-content';
        contentDiv.textContent = text;
        messageDiv.appendChild(contentDiv);

        if (type !== 'system-message') {
            const timeDiv = document.createElement('div');
            timeDiv.className = 'message-time';
            const time = createdAt ? new Date(createdAt) : new Date();
            timeDiv.textContent = formatTime(time);
            messageDiv.appendChild(timeDiv);
        }

        return messageDiv;
    };

    const addMessage = (text, type = 'user', createdAt = null) => {
        const messageElement = createMessageElement(text, type, createdAt);
        messagesContainer.appendChild(messageElement);
        scrollToBottom();
    };

    const addSystemMessage = (text) => {
        addMessage(text, 'system-message');
    };

    const loadMessages = async (isInitial = false) => {
        if (isSending) return;

        try {
            let url = '/api/chat/messages';
            if (lastMessageId) {
                url += `?after_id=${lastMessageId}`;
            }
            
            const resp = await fetch(url);
            const result = await resp.json();

            if (result.success && result.data) {
                const messages = result.data;
                
                if (messages.length > 0) {
                    if (isInitial || !lastMessageId) {
                        // Initial load - display all
                        messagesContainer.innerHTML = '';
                        messages.forEach(msg => {
                            const type = msg.sender_type === 'staff' ? 'support' : msg.sender_type;
                            addMessage(msg.content, type, msg.created_at);
                        });
                    } else {
                        // Incremental - append new messages
                        console.log(`[Chat] New messages received: ${messages.length}`);
                        messages.forEach(msg => {
                            const type = msg.sender_type === 'staff' ? 'support' : msg.sender_type;
                            addMessage(msg.content, type, msg.created_at);
                        });
                    }
                    
                    // Update last message ID
                    lastMessageId = messages[messages.length - 1].id;
                    console.log(`[Chat] Last message ID: ${lastMessageId}, Total messages loaded`);
                } else if (isInitial) {
                    console.log('[Chat] No messages found');
                }
            }
        } catch (e) {
            console.error('[Chat] Error loading messages:', e);
        }
    };

    const sendMessage = async () => {
        const text = messageInput.value.trim();
        if (!text) return;

        isSending = true;
        try {
            const resp = await fetch('/api/chat/message', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ content: text })
            });
            const result = await resp.json();

            if (result.success) {
                console.log('[Chat] Message sent');
                messageInput.value = '';
                autoResizeTextarea();
                await loadMessages();
            } else {
                console.error('[Chat] Failed to send:', result.error);
                alert(result.error || 'Failed to send message');
            }
        } catch (e) {
            console.error('[Chat] Error sending message:', e);
            alert('Failed to send message');
        } finally {
            isSending = false;
        }
    };

    const autoResizeTextarea = () => {
        messageInput.style.height = 'auto';
        const scrollHeight = messageInput.scrollHeight;
        messageInput.style.height = Math.min(scrollHeight, 100) + 'px';
    };

    const startSync = () => {
        console.log('[Chat] Starting sync...');
        loadMessages(true);
        syncTimer = setInterval(() => loadMessages(false), syncInterval);
    };

    const stopSync = () => {
        if (syncTimer) {
            clearInterval(syncTimer);
            syncTimer = null;
        }
    };

    sendBtn.addEventListener('click', sendMessage);

    messageInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
            if (e.shiftKey) {
                return;
            }
            e.preventDefault();
            sendMessage();
        }
    });

    messageInput.addEventListener('input', autoResizeTextarea);

    messageInput.addEventListener('focus', () => {
        document.body.classList.add('message-input-focused');
    });

    messageInput.addEventListener('blur', () => {
        document.body.classList.remove('message-input-focused');
    });

    const initializeChat = async () => {
        console.log('[Chat] Initializing...');
        updateConnectionStatus('connecting');
        await loadMessages(true);
        startSync();
        updateConnectionStatus('connected');
        console.log('[Chat] Ready');
        autoResizeTextarea();
        messageInput.focus();
    };

    const updateConnectionStatus = (status) => {
        connectionStatus.className = 'connection-status';
        const statusText = connectionStatus.querySelector('.status-text');

        switch (status) {
            case 'connected':
                connectionStatus.classList.add('connected');
                statusText.textContent = 'Online';
                break;
            case 'connecting':
                statusText.textContent = 'Connecting...';
                break;
            case 'disconnected':
                connectionStatus.classList.add('disconnected');
                statusText.textContent = 'Offline';
                break;
        }
    };

    document.addEventListener('DOMContentLoaded', initializeChat);

    window.addEventListener('beforeunload', stopSync);
})();
