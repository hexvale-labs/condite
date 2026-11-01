/*
 * Vencord, a Discord client mod
 * Copyright (c) 2026 Condite contributors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

export const uploadPatch = {
    find: "async uploadFiles(",
    replacement: {
        match: /async uploadFiles\((\i)\){/,
        replace:
            "$&if(!$self.isDiscordBatch($1))return $self.routeUploads($1,u=>this.uploadFiles(u));"
    }
};

export const navigationPatch = {
    find: '.FRIENDS},"friends"',
    replacement: {
        match: /\.FRIENDS\},"friends"\)/,
        replace: "$&,$self.renderUploadsNavigation()"
    }
};
