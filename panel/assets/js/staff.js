(() => {
    'use strict';

    let pollTimer = null;
    const pollInterval = 5000;

    const loadChats = async () => {
        const tbody = document.getElementById('chats-body');
        if (!tbody) return;

        try {
            const resp = await fetch('/api/staff/chats');
            const result = await resp.json();

            if (!result.success) {
                tbody.innerHTML = `<tr><td colspan="4" style="color: red;">${result.error || 'Error loading chats'}</td></tr>`;
                return;
            }

            const chats = result.data || [];

            if (chats.length === 0) {
                tbody.innerHTML = '<tr><td colspan="4" class="no-chats">No chats yet</td></tr>';
                return;
            }

            tbody.innerHTML = chats.map(chat => `
                <tr>
                    <td>${chat.user_id}</td>
                    <td>${chat.last_message}</td>
                    <td>${chat.last_time}</td>
                    <td><a href="/staff/chat/${chat.user_id}" class="action-btn">Open</a></td>
                </tr>
            `).join('');
        } catch (e) {
            console.error('[Staff] Failed to load chats:', e);
            tbody.innerHTML = '<tr><td colspan="4">Error loading chats</td></tr>';
        }
    };

    document.addEventListener('DOMContentLoaded', () => {
        loadChats();
        pollTimer = setInterval(loadChats, pollInterval);
    });
})();
