import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
public class MessageBoxStateWriteScanEvidence extends GhidraScript {
 public void run() throws Exception {
  Listing l=currentProgram.getListing();
  Address start=toAddr(0x004a6000L), end=toAddr(0x004a8000L);
  InstructionIterator it=l.getInstructions(start,true);
  while(it.hasNext()){
   Instruction ins=it.next(); if(ins.getAddress().compareTo(end)>0) break;
   String s=ins.toString().toLowerCase();
   if(s.contains("0xc4")||s.contains("[ecx + 0xc4]")||s.contains("[esi + 0xc4]")||s.contains("[edi + 0xc4]")){
    Function f=getFunctionContaining(ins.getAddress());
    println(ins.getAddress()+" "+ins+" FUNC="+(f==null?"<none>":f.getName()+"@"+f.getEntryPoint()));
   }
  }
 }
}