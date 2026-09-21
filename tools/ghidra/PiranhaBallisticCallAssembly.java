import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.Reference;
public class PiranhaBallisticCallAssembly extends GhidraScript {
  @Override public void run() throws Exception {
    disassemble(toAddr(0x004676F0L));
    Function f=getFunctionAt(toAddr(0x004676F0L));
    if(f==null){try{f=createFunction(toAddr(0x004676F0L),"evidence_4676f0");}catch(Exception ignored){}}
    if(f==null)f=getFunctionContaining(toAddr(0x004676F0L));
    InstructionIterator it=currentProgram.getListing().getInstructions(f.getBody(),true);
    while(it.hasNext()){
      Instruction ins=it.next();
      boolean hit=false;
      for(Reference ref:ins.getReferencesFrom()) if(ref.getToAddress().equals(toAddr(0x0041EC80L))) hit=true;
      if(hit){
        Address a=ins.getAddress();
        println("CALL="+a);
        Instruction cur=ins;
        for(int i=0;i<18;i++){cur=cur.getPrevious(); if(cur==null)break;}
        for(int i=0;i<32 && cur!=null;i++,cur=cur.getNext()) println(cur.getAddress()+"  "+cur);
      }
    }
  }
}