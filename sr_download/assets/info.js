'use strict';

function fetchData() {
    // 获取输入框中的 ID
    const dataId = document.getElementById("dataId").value;
    if (!dataId) {
        alert("请输入 ID");
        return;
    }
    if (dataId < 76858) {
        alert("ID 不能小于 76858 (这个是目前最小的 ID)");
        return;
    }
    // 发送请求
    fetch(`/info/${dataId}`)
        .then((response) => response.json())
        .then((data) => {
            // 获取结果显示区域
            const resultDisplay =
                document.querySelector(".result-display");
            // 清空结果显示区域
            resultDisplay.innerHTML = "";
            // 创建结果显示区域的元素
            const resultTitle = document.createElement("div");
            resultTitle.classList.add("title");
            resultTitle.innerText = "请求结果";
            resultDisplay.appendChild(resultTitle);
            // 先判断数据拿没拿到
            if (data["code"] !== 200) {
                // 没拿到
                const resultContent = document.createElement("div");
                resultContent.innerText = data["msg"];
                resultDisplay.appendChild(resultContent);
            } else {
                // 拿到了
                // 创建结果显示区域的元素
                const resultContent = document.createElement("div");
                const inner_data = data["data"];
                // 添加数据
                resultContent.innerHTML = `<div>id: ${inner_data["save_id"]}</div>
            <div>类型: ${inner_data["save_type"]}</div>
            <div>长度: ${inner_data["len"]}</div>
            <div>xml校验: ${inner_data["xml_tested"]}</div>
            <div>blake hash: <span class="monospace">${inner_data["blake_hash"]}</span></div>`;
                resultDisplay.appendChild(resultContent);
            }
        });
}
