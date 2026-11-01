// SPDX-License-Identifier: GPL-3.0-or-later
import { Button } from "@components/Button";
import type { CloudUpload, RenderModalProps } from "@vencord/discord-types";
import { closeModal, Modal, openModal, React, useState } from "@webpack/common";

import { conditeMaxBytes } from "./limits";
import type { Provider } from "./routing";

const pending = new Set<() => void>();

function Chooser({
    uploads,
    failure,
    finish,
    ...props
}: RenderModalProps & {
    uploads: CloudUpload[];
    failure: boolean;
    finish(value: Provider[] | null): void;
}) {
    const [choices, setChoices] = useState<Provider[]>(
        uploads.map(u =>
            u.item.file.size <= conditeMaxBytes ? "condite" : "discord"
        )
    );
    const single = uploads.length === 1;
    const eligible = uploads.some(u => u.item.file.size <= conditeMaxBytes);
    return (
        <Modal
            {...props}
            title={
                failure
                    ? "Discord upload failed"
                    : single
                      ? "Upload file"
                      : "Choose where to upload"
            }
            subtitle={
                failure
                    ? "Upload the failed files to Condite instead, or keep the attachments to retry in Discord."
                    : "Condite adds a download link to the original channel draft."
            }
            actions={
                single
                    ? undefined
                    : [
                          {
                              text: failure
                                  ? "Upload selected to Condite"
                                  : "Upload files",
                              variant: "primary",
                              disabled: failure && !eligible,
                              onClick: () => finish(choices)
                          },
                          {
                              text: failure ? "Keep attachments" : "Cancel",
                              variant: "secondary",
                              onClick: () => finish(null)
                          }
                      ]
            }
        >
            <div className="condite-chooser">
                {uploads.map((upload, index) => (
                    <div
                        className="condite-choice-row"
                        key={`${upload.id}-${index}`}
                    >
                        <div className="condite-file-name">
                            <strong>{upload.filename}</strong>
                            <small>
                                {upload.item.file.size.toLocaleString()} bytes
                            </small>
                        </div>
                        {!single && (
                            <label>
                                Upload to{" "}
                                <select
                                    aria-label={`Upload ${upload.filename} to`}
                                    value={choices[index]}
                                    onChange={event =>
                                        setChoices(previous =>
                                            previous.map((choice, i) =>
                                                i === index
                                                    ? (event.target
                                                          .value as Provider)
                                                    : choice
                                            )
                                        )
                                    }
                                >
                                    <option
                                        value="condite"
                                        disabled={
                                            upload.item.file.size >
                                            conditeMaxBytes
                                        }
                                    >
                                        Condite
                                    </option>
                                    <option value="discord">
                                        {failure
                                            ? "Keep attachment"
                                            : "Discord"}
                                    </option>
                                </select>
                            </label>
                        )}
                    </div>
                ))}
                {uploads.some(u => u.item.file.size > conditeMaxBytes) && (
                    <p className="condite-muted">
                        Condite currently accepts files up to 100 MiB.
                    </p>
                )}
                {single && (
                    <div className="condite-actions">
                        <Button
                            disabled={!eligible}
                            onClick={() => finish(["condite"])}
                        >
                            Upload to Condite
                        </Button>
                        <Button
                            variant="secondary"
                            onClick={() => finish(failure ? null : ["discord"])}
                        >
                            {failure ? "Keep attachment" : "Upload to Discord"}
                        </Button>
                    </div>
                )}
            </div>
        </Modal>
    );
}

export function chooseProviders(
    uploads: CloudUpload[],
    failure = false
): Promise<Provider[] | null> {
    return new Promise(resolve => {
        let done = false;
        const finish = (value: Provider[] | null) => {
            if (done) return;
            done = true;
            pending.delete(cancel);
            resolve(value);
            closeModal(key);
        };
        const cancel = () => finish(null);
        pending.add(cancel);
        const key = openModal(
            props => (
                <Chooser
                    {...props}
                    uploads={uploads}
                    failure={failure}
                    finish={finish}
                />
            ),
            {
                onCloseCallback: () => {
                    if (!done) {
                        done = true;
                        pending.delete(cancel);
                        resolve(null);
                    }
                }
            }
        );
    });
}

export function closeChoosers() {
    for (const cancel of [...pending]) cancel();
}
