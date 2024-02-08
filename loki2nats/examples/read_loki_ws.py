#!/usr/bin/env python3
import sys

from collections import namedtuple
from datetime import datetime
from json import loads
from ssl import CERT_REQUIRED, Purpose, create_default_context
from socket import AF_INET, SOCK_STREAM, SOL_SOCKET, SO_KEEPALIVE, socket
from time import time
from urllib.parse import urlparse, quote

from websocket import create_connection, enableTrace


class WebsocketBuilder:
    def __init__(self, websocket_url):
        self.websocket_url = websocket_url

    def get_ssl_context(self):
        if not hasattr(self, '_ssl_context'):
            context = create_default_context(
                Purpose.SERVER_AUTH)
            context.verify_mode = CERT_REQUIRED
            context.check_hostname = True
            context.load_default_certs()

            self._ssl_context = context
        return self._ssl_context

    def create_connection(self):
        hostname = urlparse(self.websocket_url).netloc
        port = 443

        sock = socket(AF_INET, SOCK_STREAM)

        sslsock = self.get_ssl_context().wrap_socket(
            sock, server_hostname=hostname)
        sslsock.setsockopt(SOL_SOCKET, SO_KEEPALIVE, 1)
        sslsock.connect((hostname, port))

        return create_connection(self.websocket_url, socket=sslsock)


class LokiMsg(namedtuple(
        'LokiMsg',
        'tenant src_loki src_host src_log nanotime raw_data struct_data')):

    def as_cloudevent_js(self):
        # https://github.com/cloudevents/spec/blob/main/cloudevents/spec.md#
        #   +#context-attributes
        #   +#event-data
        dt = datetime.utcfromtimestamp(int(self.nanotime) / 1_000_000_000)
        rfc3339_time = dt.isoformat() + 'Z'
        event = {
            # Producers MUST ensure that source + id is unique for each
            # distinct event.
            'id': '{}/{}/{}'.format(
                self.nanotime, self.src_host, self.src_log),
            # An absolute URI is RECOMMENDED.
            'source': 'https+loki://{}/{}/osso-ops/{}'.format(
                self.src_loki, self.src_host, self.tenant),
            # Compliant event producers MUST use a value of 1.0 when ...
            'specversion': '1.0',
            # SHOULD be prefixed with a reverse-DNS name.
            'type': 'nl.osso.spec.alpha.loki2nats',
            # [...] a JSON-format event with no datacontenttype is exactly
            # equivalent to one with datacontenttype="application/json".
            # #'datacontenttype': 'application/json',
            # "E.g. mynewfile.jpg", or in our case the filename/systemd_unit.
            'subject': self.src_log,
            # Optional. But we do have a time, so use it.
            'time': rfc3339_time,
        }
        if self.struct_data:
            assert not self.raw_data, self
            event['datacontenttype'] = 'application/json'
            event['data'] = self.struct_data
        else:
            event['datacontenttype'] = 'text/plain'
            event['data'] = self.raw_data

        return event


class LokiStreamReader:
    def __init__(self, ws, loki_id):
        self.ws = ws
        self.loki_id = loki_id

    def messages(self):
        "Returns an iterable of LokiMsg objects"
        data_keys = set(['streams'])
        stream_keys = set(['stream', 'values'])

        while True:
            data = self.ws.recv()
            if not data:
                break

            data = loads(data)

            dropped_entries = data.pop('dropped_entries', None)
            handled_entries = 0

            assert set(data.keys()) == data_keys, data.keys()
            streams = data['streams']
            assert isinstance(streams, list), streams

            for stream in streams:
                assert set(stream.keys()) == stream_keys, streams.keys()
                labels = stream['stream']
                values = stream['values']
                assert isinstance(values, list)

                for value in values:
                    handled_entries += 1
                    yield self.make_lokimsg(labels, value)

            print('HANDLED', handled_entries, file=sys.stderr)
            if dropped_entries:
                print('DROPPED (!)', len(dropped_entries), file=sys.stderr)

    def extract_log_sources(self, labels):
        tenant = labels['tenant']
        src_host = labels['host']

        if 'filename' in labels:
            assert 'systemd_unit' not in labels, labels
            src_log = labels['filename']
        elif 'systemd_unit' in labels:
            assert 'file' not in labels, labels
            src_log = labels['systemd_unit']
        else:
            assert False, labels

        return tenant, src_host, src_log

    def make_lokimsg(self, labels, values):
        nanotime, data = values
        src_loki = self.loki_id
        tenant, src_host, src_log = self.extract_log_sources(labels)
        try:
            struct_data = loads(data)
        except ValueError:
            struct_data, raw_data = None, data
        else:
            raw_data = None
        return LokiMsg(
            tenant, src_loki, src_host, src_log, nanotime, raw_data,
            struct_data)


def main():
    hostname = sys.argv[1]
    filter_ = sys.argv[2]   # {host!=""}, {host!="", tenant="sometenant"}

    filter_ = quote(filter_)
    limit = 1
    unixtime_ms = int(time() * 1000)
    websocket_url = (
        f'wss://{hostname}/loki/api/v1/tail?limit={limit}&query={filter_}'
        f'&query_log=5&start={unixtime_ms}000000')
    certfile = './loki_client.crt'
    keyfile = './loki_client.key'

    if 0:
        enableTrace(True)

    websocket_builder = WebsocketBuilder(websocket_url)
    websocket_builder.get_ssl_context().load_cert_chain(
        certfile=certfile, keyfile=keyfile)
    ws = websocket_builder.create_connection()

    rd = LokiStreamReader(ws, loki_id=hostname)
    for idx, logmsg in enumerate(rd.messages()):
        print(idx, logmsg.as_cloudevent_js())
        print()


if __name__ == '__main__':
    main()
