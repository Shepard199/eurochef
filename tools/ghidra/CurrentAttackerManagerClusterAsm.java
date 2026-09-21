import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;

public class CurrentAttackerManagerClusterAsm extends GhidraScript {
 @Override public void run() throws Exception {
  Address start=toAddr(0x00455F00L);
  disassemble(start);
  InstructionIterator it=currentProgram.getListing().getInstructions(start,true);
  int n=0;
  while(it.hasNext()&&n<2600){
   Instruction ins=it.next();
   long a=ins.getAddress().getOffset();
   if(a>=0x00456800L)break;
   String s=ins.toString().toLowerCase();
   if(s.contains("+ 0x34")||s.contains("+0x34")||s.contains("0x34]")||s.contains("0x34,")){
     println(ins.getAddress()+"  "+ins);
   }
   n++;
  }
 }
}