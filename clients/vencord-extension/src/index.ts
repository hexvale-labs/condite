// SPDX-License-Identifier: GPL-3.0-or-later
import definePlugin from "@utils/types";
import type { CloudUpload } from "@vencord/discord-types";
import { showToast } from "@webpack/common";

export default definePlugin({
    name: "Condite",
    description: "Upload interception PoC: shows filenames in a toast and blocks Discord uploads.",
    authors: [],

    patches: [{
        find: "async uploadFiles(",
        replacement: {
            match: /async uploadFiles\((\i)\){/,
            replace: "$&return $self.blockUploads($1);"
        }
    }],

    blockUploads(uploads: CloudUpload[]) {
        const filenames = uploads.map(upload => upload.filename || upload.item?.file?.name || "Unknown file");
        showToast({
            text: `Condite PoC blocked upload: ${filenames.join(", ") || "Empty upload batch"}`,
            variant: "critical",
            duration: 6000
        });
    }
});
