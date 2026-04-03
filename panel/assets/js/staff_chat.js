(() => {
    'use strict';

    let syncTimer = null;
    let currentUserId = null;
    let lastMessageId = null;
    let isSending = false;
    const syncInterval = 3000;

    const getUserIdFromUrl = () => {
        const path = window.location.pathname;
        const match = path.match(/\/staff\/chat\/([^\/]+)/);
        return match ? match[1] : null;
    };

    const formatTime = (dateStr) => {
        const date = new Date(dateStr);
        return date.toLocaleTimeString('en-US', { 
            hour: 'numeric', 
            minute: '2-digit',
            hour12: true 
        });
    };

    const escapeHtml = (text) => {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    };

    const loadMessages = async (isInitial = false) => {
        if (isSending) return;

        const userId = getUserIdFromUrl();
        if (!userId) {
            document.getElementById('chat-messages').innerHTML = '<div>Invalid user ID</div>';
            return;
        }

        currentUserId = userId;
        document.getElementById('user-id').textContent = userId;

        try {
            let url = `/api/chat/messages?user_id=${userId}`;
            if (lastMessageId) {
                url += `&after_id=${lastMessageId}`;
            }
            
            const resp = await fetch(url);
            const result = await resp.json();

            if (!result.success) {
                console.error('[StaffChat] Error loading messages:', result.error);
                return;
            }

            const messages = result.data || [];
            
            if (messages.length > 0) {
                if (isInitial || !lastMessageId) {
                    // Initial load
                    if (messages.length === 0) {
                        document.getElementById('chat-messages').innerHTML = '<div>No messages yet</div>';
                        return;
                    }
                    document.getElementById('chat-messages').innerHTML = messages.map(msg => `
                        <div class="message ${msg.sender_type}">
                            <div class="message-content">${escapeHtml(msg.content)}</div>
                            <div class="message-time">${formatTime(msg.created_at)}</div>
                        </div>
                    `).join('');
                } else {
                    // Incremental - append new
                    console.log(`[StaffChat] New messages: ${messages.length}`);
                    const container = document.getElementById('chat-messages');
                    messages.forEach(msg => {
                        const div = document.createElement('div');
                        div.className = `message ${msg.sender_type}`;
                        div.innerHTML = `
                            <div class="message-content">${escapeHtml(msg.content)}</div>
                            <div class="message-time">${formatTime(msg.created_at)}</div>
                        `;
                        container.appendChild(div);
                    });
                }
                
                lastMessageId = messages[messages.length - 1].id;
                document.getElementById('chat-messages').scrollTop = 
                    document.getElementById('chat-messages').scrollHeight;
            }
        } catch (e) {
            console.error('[StaffChat] Failed to load messages:', e);
        }
    };

    const sendReply = async (content) => {
        const userId = getUserIdFromUrl();
        if (!userId || !content.trim()) return;

        isSending = true;
        try {
            const resp = await fetch(`/api/staff/chats/${userId}/message`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ content: content.trim() })
            });
            const result = await resp.json();

            if (result.success) {
                console.log('[StaffChat] Reply sent');
                document.getElementById('reply-input').value = '';
                await loadMessages();
            } else {
                alert(result.error || 'Failed to send message');
            }
        } catch (e) {
            console.error('[StaffChat] Failed to send reply:', e);
            alert('Failed to send message');
        } finally {
            isSending = false;
        }
    };

    const startSync = () => {
        console.log('[StaffChat] Starting sync...');
        loadMessages(true);
        syncTimer = setInterval(() => loadMessages(false), syncInterval);
    };

    const stopSync = () => {
        if (syncTimer) {
            clearInterval(syncTimer);
            syncTimer = null;
        }
    };

    document.getElementById('reply-form').addEventListener('submit', (e) => {
        e.preventDefault();
        const input = document.getElementById('reply-input');
        sendReply(input.value);
    });

    document.addEventListener('DOMContentLoaded', startSync);
    window.addEventListener('beforeunload', stopSync);
})();
